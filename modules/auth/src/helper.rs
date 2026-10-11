//! polkit's own helper, which checks a password with PAM and, when it's
//! right, tells polkitd that the cookie's authentication succeeded. It runs
//! as root: polkitd starts it for each connection to
//! `/run/polkit/agent-helper.socket`, or older polkits have it setuid.
//!
//! The conversation is lines: the user's name (on the socket) and the
//! cookie go in; `PAM_PROMPT_ECHO_OFF <prompt>` asks for the password,
//! `PAM_ERROR_MSG` and `PAM_TEXT_INFO` say something, and `SUCCESS` or
//! `FAILURE` end it. The helper escapes each line as C does.

use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use zeroize::Zeroizing;

const SOCKET: &str = "/run/polkit/agent-helper.socket";

/// Where a setuid helper may be, for a polkit without the socket.
const HELPERS: [&str; 4] = [
    "/run/wrappers/bin/polkit-agent-helper-1",
    "/usr/lib/polkit-1/polkit-agent-helper-1",
    "/usr/libexec/polkit-agent-helper-1",
    "/usr/lib/policykit-1/polkit-agent-helper-1",
];

/// The longest a check may take, PAM's delay after a wrong password too.
const WAIT: Duration = Duration::from_secs(30);

/// Checks `password` for `user`, for polkitd's `cookie`. `Err` holds what
/// PAM said, if anything.
pub async fn check(user: &str, cookie: &str, password: &str) -> Result<(), Vec<String>> {
    let conversation = async {
        if Path::new(SOCKET).exists()
            && let Ok(socket) = tokio::net::UnixStream::connect(SOCKET).await
        {
            let (reader, mut writer) = socket.into_split();
            writer
                .write_all(format!("{user}\n").as_bytes())
                .await
                .map_err(|error| vec![error.to_string()])?;
            return converse(reader, writer, cookie, password).await;
        }
        let helper = HELPERS
            .iter()
            .find(|helper| Path::new(helper).exists())
            .ok_or_else(|| vec!["polkit's helper isn't installed".to_owned()])?;
        let mut child = tokio::process::Command::new(helper)
            .arg(user)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| vec![error.to_string()])?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(vec!["can't talk to polkit's helper".to_owned()]);
        };
        converse(stdout, stdin, cookie, password).await
    };
    tokio::time::timeout(WAIT, conversation)
        .await
        .unwrap_or_else(|_| Err(vec!["The check took too long".to_owned()]))
}

async fn converse(
    reader: impl AsyncRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
    cookie: &str,
    password: &str,
) -> Result<(), Vec<String>> {
    let failed = |error: std::io::Error| vec![error.to_string()];
    writer
        .write_all(format!("{cookie}\n").as_bytes())
        .await
        .map_err(failed)?;
    let mut said = Vec::new();
    let mut answered = false;
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await.map_err(failed)? {
        let line = unescape(&line);
        if let Some(prompt) = line.strip_prefix("PAM_PROMPT_ECHO_OFF ") {
            // One password: a second question, like a one-time code, isn't
            // something this prompt can answer.
            if answered {
                said.push(format!(
                    "It also asks: {}, which Mochi can't answer",
                    prompt.trim()
                ));
                return Err(said);
            }
            answered = true;
            let reply = Zeroizing::new(format!("{password}\n"));
            writer.write_all(reply.as_bytes()).await.map_err(failed)?;
        } else if let Some(prompt) = line.strip_prefix("PAM_PROMPT_ECHO_ON ") {
            said.push(format!(
                "It asks: {}, which Mochi can't answer",
                prompt.trim()
            ));
            return Err(said);
        } else if let Some(message) = line
            .strip_prefix("PAM_ERROR_MSG ")
            .or_else(|| line.strip_prefix("PAM_TEXT_INFO "))
        {
            let message = message.trim();
            if !message.is_empty() {
                said.push(message.to_owned());
            }
        } else if line.starts_with("SUCCESS") {
            return Ok(());
        } else {
            // FAILURE, or a line no helper should say.
            return Err(said);
        }
    }
    Err(said)
}

/// Undoes the helper's escaping, as GLib's `g_strcompress`: `\n`, `\t`,
/// `\\`, `\"` and octal `\NNN`.
fn unescape(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if byte != b'\\' || index >= bytes.len() {
            out.push(byte);
            continue;
        }
        let escaped = bytes[index];
        index += 1;
        out.push(match escaped {
            b'b' => 0x08,
            b'f' => 0x0c,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'v' => 0x0b,
            b'0'..=b'7' => {
                let mut value = u32::from(escaped - b'0');
                for _ in 0..2 {
                    match bytes.get(index) {
                        Some(digit @ b'0'..=b'7') => {
                            value = value * 8 + u32::from(digit - b'0');
                            index += 1;
                        }
                        _ => break,
                    }
                }
                value as u8
            }
            other => other,
        });
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_as_glib_does() {
        assert_eq!(unescape(r"Password: "), "Password: ");
        assert_eq!(unescape(r#"a\nb\tc\\d\"e"#), "a\nb\tc\\d\"e");
        assert_eq!(unescape(r"caf\303\251"), "café");
        assert_eq!(unescape(r"end\"), "end\\");
    }

    /// A helper played by a test: what it says, and what it got.
    async fn talk(says: &str, password: &str) -> (Result<(), Vec<String>>, String) {
        let (agent, helper) = tokio::io::duplex(4096);
        let (agent_read, agent_write) = tokio::io::split(agent);
        let (helper_read, mut helper_write) = tokio::io::split(helper);
        helper_write.write_all(says.as_bytes()).await.unwrap();
        drop(helper_write);
        let result = converse(agent_read, agent_write, "cookie-1", password).await;
        let mut got = String::new();
        let mut reader = BufReader::new(helper_read);
        while let Ok(read) = reader.read_line(&mut got).await {
            if read == 0 {
                break;
            }
        }
        (result, got)
    }

    #[tokio::test]
    async fn sends_the_cookie_then_the_password_once() {
        let (result, got) = talk("PAM_PROMPT_ECHO_OFF Password: \nSUCCESS\n", "hunter2").await;
        assert_eq!(result, Ok(()));
        assert_eq!(got, "cookie-1\nhunter2\n");
    }

    #[tokio::test]
    async fn says_what_pam_said_when_it_fails() {
        let (result, _) = talk(
            "PAM_PROMPT_ECHO_OFF Password: \nPAM_ERROR_MSG Account locked\nFAILURE\n",
            "x",
        )
        .await;
        assert_eq!(result, Err(vec!["Account locked".to_owned()]));
    }

    #[tokio::test]
    async fn a_second_question_is_not_answered() {
        let (result, got) = talk(
            "PAM_PROMPT_ECHO_OFF Password: \nPAM_PROMPT_ECHO_OFF Code: \n",
            "x",
        )
        .await;
        assert!(result.is_err());
        assert_eq!(got, "cookie-1\nx\n");
    }
}

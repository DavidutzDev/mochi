//! Script providers: a program from `config.toml`, run once per query with
//! the query as its last argument and in `MOCHI_QUERY`. What it prints is
//! the answer, as JSON lines. A newer query kills it, and so does its
//! timeout.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// More than any list shows.
const MAX_OUTPUT: u64 = 1 << 20;

/// Runs `command` with `argument` after it and returns what it printed.
pub async fn run(
    command: &[String],
    argument: &str,
    timeout: Duration,
    env: (&str, &str),
) -> Result<String, String> {
    let (program, args) = command.split_first().ok_or("the command is empty")?;
    let mut child = Command::new(program)
        .args(args)
        .arg(argument)
        .env(env.0, env.1)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // Dropped when a newer query comes: it goes too.
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    let stdout = child.stdout.take().ok_or("no output")?;
    let read = async {
        let mut output = String::new();
        stdout
            .take(MAX_OUTPUT)
            .read_to_string(&mut output)
            .await
            .map_err(|error| error.to_string())?;
        let status = child.wait().await.map_err(|error| error.to_string())?;
        if status.success() {
            Ok(output)
        } else {
            Err(format!("{program} {status}"))
        }
    };
    tokio::time::timeout(timeout, read)
        .await
        .map_err(|_| format!("{program} took longer than {} ms", timeout.as_millis()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Vec<String> {
        vec!["sh".into(), "-c".into(), script.into(), "sh".into()]
    }

    #[tokio::test]
    async fn passes_the_query_and_returns_the_output() {
        let output = run(
            &sh(r#"printf '{"title": "%s"}\n' "$1" "$MOCHI_QUERY""#),
            "rust",
            Duration::from_secs(2),
            ("MOCHI_QUERY", "also"),
        )
        .await
        .unwrap();
        assert_eq!(output, "{\"title\": \"rust\"}\n{\"title\": \"also\"}\n");
    }

    #[tokio::test]
    async fn fails_on_errors_and_slowness() {
        let error = run(&sh("exit 3"), "", Duration::from_secs(2), ("Q", ""))
            .await
            .unwrap_err();
        assert!(error.contains("exit status: 3"), "{error}");
        let error = run(&sh("sleep 5"), "", Duration::from_millis(100), ("Q", ""))
            .await
            .unwrap_err();
        assert!(error.contains("took longer"), "{error}");
        let error = run(
            &["/nonexistent/program".into()],
            "",
            Duration::from_secs(1),
            ("Q", ""),
        )
        .await
        .unwrap_err();
        assert!(error.contains("cannot run"), "{error}");
    }
}

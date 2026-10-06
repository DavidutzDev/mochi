//! Starting apps so they don't belong to mochid: restarting Mochi must never
//! take an app down with it.

use std::process::Stdio;

use mochi_core::process;
use tokio::process::Command;

use crate::entries::{App, AppAction, installed};

/// How apps get started, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `uwsm app`: a systemd unit per app, the way a uwsm session expects.
    /// uwsm reads the desktop entry itself, terminal apps included.
    Uwsm,
    /// `systemd-run --user --scope`: its own scope in `app.slice`.
    SystemdRun,
    /// A plain process in its own process group.
    Direct,
}

impl Method {
    pub async fn detect() -> Self {
        if installed("uwsm") && succeeds(Command::new("uwsm").args(["check", "is-active"])).await {
            Self::Uwsm
        } else if installed("systemd-run") {
            Self::SystemdRun
        } else {
            Self::Direct
        }
    }
}

async fn succeeds(command: &mut Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

/// Starts an app, or one of its actions. `terminal` runs terminal apps, like
/// `["foot", "-e"]`.
pub fn launch(
    method: Method,
    app: &App,
    action: Option<&AppAction>,
    terminal: &[String],
) -> Result<(), String> {
    let argv = match method {
        Method::Uwsm => {
            let entry = match action {
                Some(action) => format!("{}:{}", app.id, action.id),
                None => app.id.clone(),
            };
            vec!["uwsm".into(), "app".into(), "--".into(), entry]
        }
        Method::SystemdRun | Method::Direct => {
            let exec = action.map_or(&app.exec, |action| &action.exec);
            let mut argv = command_line(exec, app)
                .ok_or_else(|| format!("cannot read the command of {}", app.id))?;
            if app.terminal {
                if terminal.is_empty() {
                    return Err(format!("{} runs in a terminal, and none is set", app.name));
                }
                argv.splice(0..0, terminal.iter().cloned());
            }
            if method == Method::SystemdRun {
                argv = process::in_app_scope(&argv);
            }
            argv
        }
    };
    // Detached, so the app's parent is systemd, not mochid: an app that
    // ends with its parent, like Discord in bubblewrap, would otherwise close
    // when Mochi stops.
    process::spawn_detached(&argv, app.path.as_deref())
}

/// Starts a command the way apps start, so it doesn't belong to mochid
/// either. With `terminal`, in that terminal, like `["foot", "-e"]`.
pub fn spawn(method: Method, argv: &[String], terminal: Option<&[String]>) -> Result<(), String> {
    let mut argv = argv.to_vec();
    if let Some(terminal) = terminal {
        if terminal.is_empty() {
            return Err("no terminal is set".into());
        }
        argv.splice(0..0, terminal.iter().cloned());
    }
    let argv = match method {
        Method::Uwsm => ["uwsm", "app", "--"]
            .into_iter()
            .map(String::from)
            .chain(argv)
            .collect(),
        Method::SystemdRun => process::in_app_scope(&argv),
        Method::Direct => argv,
    };
    process::spawn_detached(&argv, None)
}

/// Turns `Exec=` into arguments: the spec's quoting, and its field codes.
/// Nothing is opened with the app, so the file and URL codes go away.
pub fn command_line(exec: &str, app: &App) -> Option<Vec<String>> {
    let mut argv = Vec::new();
    for word in words(exec)? {
        match word.as_str() {
            "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => {}
            "%i" => {
                if let Some(icon) = &app.icon {
                    argv.extend(["--icon".to_owned(), icon.clone()]);
                }
            }
            _ => argv.push(expand(&word, app)),
        }
    }
    (!argv.is_empty()).then_some(argv)
}

/// Field codes inside a word: `%c` the name, `%k` the file, `%%` a percent
/// sign, and nothing for the rest.
fn expand(word: &str, app: &App) -> String {
    let mut out = String::with_capacity(word.len());
    let mut chars = word.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('c') => out.push_str(&app.name),
            Some('k') => out.push_str(&app.file.to_string_lossy()),
            _ => {}
        }
    }
    out
}

/// Splits on spaces outside double quotes. Inside quotes, a backslash makes
/// the next `"`, `` ` ``, `$` or `\` literal. `None` for an unclosed quote.
fn words(exec: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => word.push(chars.next()?),
            ' ' | '\t' if !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            other => {
                word.push(other);
                started = true;
            }
        }
    }
    if quoted {
        return None;
    }
    if started {
        words.push(word);
    }
    Some(words)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn app(exec: &str) -> App {
        App {
            id: "a.desktop".into(),
            name: "My App".into(),
            generic: None,
            comment: None,
            icon: Some("my-icon".into()),
            exec: exec.into(),
            terminal: false,
            keywords: Vec::new(),
            path: None,
            actions: Vec::new(),
            file: PathBuf::from("/apps/a.desktop"),
        }
    }

    fn argv(exec: &str) -> Option<Vec<String>> {
        command_line(exec, &app(exec))
    }

    #[test]
    fn drops_file_codes_and_fills_in_the_rest() {
        assert_eq!(
            argv("firefox --name firefox %U").unwrap(),
            ["firefox", "--name", "firefox"]
        );
        assert_eq!(
            argv("app %i --title=%c --from %k 100%%").unwrap(),
            [
                "app",
                "--icon",
                "my-icon",
                "--title=My App",
                "--from",
                "/apps/a.desktop",
                "100%"
            ]
        );
    }

    #[test]
    fn follows_the_quoting_rules() {
        assert_eq!(
            argv(r#"sh -c "echo \"hi there\" \$HOME" ''"#).unwrap(),
            ["sh", "-c", r#"echo "hi there" $HOME"#, "''"]
        );
        assert_eq!(
            argv(r#""/opt/My App/run"  --x"#).unwrap(),
            ["/opt/My App/run", "--x"]
        );
        assert_eq!(argv(r#"a """#).unwrap(), ["a", ""]);
        assert_eq!(argv(r#"broken "quote"#), None);
        assert_eq!(argv("%U"), None);
    }
}

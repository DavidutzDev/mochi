//! How this Mochi was installed, which decides how it updates: the install
//! script updates in place, a package manager or Nix with a command the page
//! shows, and a build from the source tree with git.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// The universal installer, as the README gives it.
pub const INSTALLER: &str = "https://raw.githubusercontent.com/DavidutzDev/mochi/main/install.sh";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Method {
    /// The install script put it in `prefix`, which it updates.
    Script { prefix: PathBuf },
    /// A package of the system's package manager.
    Package { manager: String, package: String },
    /// The Nix store: the flake that names it updates it.
    Nix,
    /// A debug build, from the source tree.
    Source { dir: PathBuf },
    /// None of those.
    Unknown,
}

/// The running daemon's executable, without the " (deleted)" Linux adds
/// once an update replaced it.
pub fn executable() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?;
    let text = path.to_string_lossy();
    Some(PathBuf::from(
        text.strip_suffix(" (deleted)").unwrap_or(&text).to_owned(),
    ))
}

impl Method {
    pub fn detect() -> Self {
        if cfg!(debug_assertions) {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .map(Path::to_path_buf)
                .unwrap_or_default();
            return Self::Source { dir };
        }
        let Some(exe) = executable() else {
            return Self::Unknown;
        };
        Self::of(&exe, &|program, args| {
            let output = std::process::Command::new(program)
                .args(args)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
    }

    /// The method for `exe`; `ask` runs a package manager's query and gives
    /// its output, `None` when it fails or isn't there.
    fn of(exe: &Path, ask: &dyn Fn(&str, &[&str]) -> Option<String>) -> Self {
        let real = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_owned());
        if real.starts_with("/nix/store") {
            return Self::Nix;
        }
        // <prefix>/bin/mochid, with <prefix>/share/mochi/install.sh beside.
        if let Some(prefix) = real.parent().and_then(Path::parent)
            && prefix.join("share/mochi/install.sh").is_file()
            && prefix.join("share/mochi/VERSION").is_file()
        {
            return Self::Script {
                prefix: prefix.to_owned(),
            };
        }
        let path = real.to_string_lossy();
        if let Some(package) = ask("pacman", &["-Qqo", &path]) {
            return Self::Package {
                manager: "pacman".into(),
                package,
            };
        }
        if let Some(owner) = ask("dpkg", &["-S", &path]) {
            let package = owner.split(':').next().unwrap_or_default().to_owned();
            return Self::Package {
                manager: "apt".into(),
                package,
            };
        }
        if let Some(package) = ask("rpm", &["-qf", "--qf", "%{NAME}", &path]) {
            return Self::Package {
                manager: "dnf".into(),
                package,
            };
        }
        Self::Unknown
    }

    /// The command that updates it, to show, copy and run in a terminal,
    /// and what to say about it.
    pub fn command(&self) -> (Vec<String>, String) {
        let words = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        match self {
            Self::Script { prefix } => (
                vec![
                    "sh".into(),
                    "-c".into(),
                    format!(
                        "curl -fsSL {INSTALLER} | sh -s -- --prefix {}",
                        shell_quote(&prefix.display().to_string())
                    ),
                ],
                format!(
                    "Installed with the install script in {}, which Update runs again.",
                    prefix.display()
                ),
            ),
            Self::Package { manager, package } if manager == "pacman" => {
                // The binary packages come from the AUR, which pacman alone
                // doesn't update.
                let helper = ["paru", "yay"]
                    .into_iter()
                    .find(|helper| mochi_core::process::installed(helper));
                match helper {
                    Some(helper) => (
                        words(&format!("{helper} -Syu {package}")),
                        format!("From the {package} package, with {helper}."),
                    ),
                    None => (
                        words("sudo pacman -Syu"),
                        format!(
                            "From the {package} package. From the AUR, it needs your AUR helper to update."
                        ),
                    ),
                }
            }
            Self::Package { manager, package } if manager == "apt" => (
                vec![
                    "sh".into(),
                    "-c".into(),
                    format!("sudo apt update && sudo apt install --only-upgrade {package}"),
                ],
                format!("From the {package} package, with apt."),
            ),
            Self::Package { package, .. } => (
                words(&format!("sudo dnf upgrade {package}")),
                format!("From the {package} package, with dnf."),
            ),
            Self::Nix => (
                vec![
                    "sh".into(),
                    "-c".into(),
                    "nix flake update mochi && sudo nixos-rebuild switch".into(),
                ],
                "From Nix. Update the mochi input of your flake, then rebuild the way you do: nixos-rebuild, home-manager or nh. Set `command` to your own to run it from here."
                    .into(),
            ),
            Self::Source { dir } => (
                vec![
                    "sh".into(),
                    "-c".into(),
                    format!(
                        "cd {} && git pull && cargo build",
                        shell_quote(&dir.display().to_string())
                    ),
                ],
                format!(
                    "A debug build from the source tree in {}: pull and build again.",
                    dir.display()
                ),
            ),
            Self::Unknown => (
                vec![
                    "sh".into(),
                    "-c".into(),
                    format!("curl -fsSL {INSTALLER} | sh"),
                ],
                "Where this Mochi comes from isn't clear. The install script installs the latest release into ~/.local."
                    .into(),
            ),
        }
    }
}

/// `text` as one word for `sh`.
pub fn shell_quote(text: &str) -> String {
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+:=@".contains(c))
    {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

/// A command as one line, to show and copy.
pub fn line(argv: &[String]) -> String {
    match argv {
        [sh, flag, script] if sh == "sh" && flag == "-c" => script.clone(),
        _ => argv
            .iter()
            .map(|word| shell_quote(word))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// The program that opens a terminal running a command: `terminal` from
/// the settings, xdg-terminal-exec, or `$TERMINAL -e`.
pub fn terminal(configured: &[String]) -> Option<Vec<String>> {
    if !configured.is_empty() {
        return Some(configured.to_vec());
    }
    if mochi_core::process::installed("xdg-terminal-exec") {
        return Some(vec!["xdg-terminal-exec".into()]);
    }
    match std::env::var("TERMINAL") {
        Ok(terminal) if !terminal.is_empty() => Some(vec![terminal, "-e".into()]),
        _ => None,
    }
}

/// Opens a terminal running `argv`, which waits for Enter at the end so
/// what it said stays readable.
pub fn run_in_terminal(terminal: &[String], argv: &[String]) -> Result<(), String> {
    let script = format!(
        "{}; status=$?; echo; if [ $status -eq 0 ]; then echo 'Done.'; else echo \"It failed ($status).\"; fi; printf 'Press Enter to close. '; read _",
        line(argv)
    );
    let mut full = terminal.to_vec();
    full.extend(["sh".into(), "-c".into(), script]);
    mochi_core::process::spawn_detached(&mochi_core::process::in_app_scope(&full), None)
}

/// Starts this daemon again once it has stopped, from `exe` with the same
/// arguments, then stops it. Under systemd, the service restarts instead.
pub fn restart(exe: &Path) -> Result<(), String> {
    if std::env::var_os("INVOCATION_ID").is_some() && mochi_core::process::installed("systemctl") {
        return mochi_core::process::spawn_detached(
            &[
                "systemctl".into(),
                "--user".into(),
                "restart".into(),
                "mochid".into(),
            ],
            None,
        );
    }
    let pid = std::process::id();
    let args: Vec<String> = std::env::args()
        .skip(1)
        .map(|arg| shell_quote(&arg))
        .collect();
    let script = format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.2; done; exec {} {}",
        shell_quote(&exe.display().to_string()),
        args.join(" ")
    );
    mochi_core::process::spawn_detached(&["sh".into(), "-c".into(), script], None)?;
    // SAFETY: kill has no memory-safety preconditions; this asks the daemon
    // to stop the way SIGTERM always does.
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str, _: &[&str]) -> Option<String> {
        None
    }

    #[test]
    fn the_install_script_is_found_by_its_files() {
        let dir = std::env::temp_dir().join(format!("mochi-updater-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("share/mochi")).unwrap();
        std::fs::write(dir.join("bin/mochid"), "").unwrap();
        assert_eq!(Method::of(&dir.join("bin/mochid"), &none), Method::Unknown);
        std::fs::write(dir.join("share/mochi/install.sh"), "").unwrap();
        std::fs::write(dir.join("share/mochi/VERSION"), "0.0.8").unwrap();
        let canonical = std::fs::canonicalize(&dir).unwrap();
        assert_eq!(
            Method::of(&dir.join("bin/mochid"), &none),
            Method::Script { prefix: canonical }
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_package_names_its_manager() {
        let pacman =
            |program: &str, _: &[&str]| (program == "pacman").then(|| "mochi-bin".to_owned());
        assert_eq!(
            Method::of(Path::new("/usr/bin/mochid"), &pacman),
            Method::Package {
                manager: "pacman".into(),
                package: "mochi-bin".into()
            }
        );
        let dpkg = |program: &str, _: &[&str]| {
            (program == "dpkg").then(|| "mochi: /usr/bin/mochid".to_owned())
        };
        let (command, _) = Method::of(Path::new("/usr/bin/mochid"), &dpkg).command();
        assert_eq!(
            line(&command),
            "sudo apt update && sudo apt install --only-upgrade mochi"
        );
    }

    #[test]
    fn commands_read_as_typed() {
        let (command, _) = Method::Script {
            prefix: PathBuf::from("/home/me/my apps"),
        }
        .command();
        assert_eq!(
            line(&command),
            format!("curl -fsSL {INSTALLER} | sh -s -- --prefix '/home/me/my apps'")
        );
        assert_eq!(
            line(&["nh".into(), "os".into(), "switch".into()]),
            "nh os switch"
        );
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }
}

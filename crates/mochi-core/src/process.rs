//! Starting programs that must outlive mochid: apps, editors, file managers.
//!
//! A program mochid starts is its child and lands in its cgroup. Stopping
//! the service ends both, and a program that watches its parent, like an
//! app in bubblewrap with `--die-with-parent`, ends with mochid even from a
//! scope of its own. So these programs go in a scope in `app.slice` when
//! `systemd-run` is there, and start through a double fork in a new session:
//! the intermediate process exits at once, and the user's systemd manager
//! adopts the program.

use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// Starts `argv` on its own, adopted by systemd rather than mochid. `dir`
/// is the working directory, or the home directory without one.
pub fn spawn_detached(argv: &[String], dir: Option<&Path>) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or("empty command")?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match dir.filter(|dir| dir.is_dir()) {
        Some(dir) => {
            command.current_dir(dir);
        }
        None => {
            if let Some(home) = std::env::var_os("HOME") {
                command.current_dir(home);
            }
        }
    }
    // SAFETY: the closure runs in the forked child before exec, where only
    // async-signal-safe calls are allowed: fork, setsid and _exit are. The
    // child forks once more and exits; the grandchild leads a new session
    // and goes on to exec, so its parent is whoever adopts orphans.
    unsafe {
        command.pre_exec(|| match libc::fork() {
            -1 => Err(io::Error::last_os_error()),
            0 => {
                libc::setsid();
                Ok(())
            }
            _ => libc::_exit(0),
        });
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start {program}: {error}"))?;
    // The intermediate process has exited by now; reap it.
    child
        .wait()
        .map(drop)
        .map_err(|error| format!("cannot start {program}: {error}"))
}

/// `argv` in a scope of its own in `app.slice`, through `systemd-run` when
/// it's installed, so stopping mochid's service leaves it alone.
pub fn in_app_scope(argv: &[String]) -> Vec<String> {
    if !installed("systemd-run") {
        return argv.to_vec();
    }
    let mut scoped: Vec<String> = [
        "systemd-run",
        "--user",
        "--scope",
        "--slice=app.slice",
        "--quiet",
        "--",
    ]
    .map(String::from)
    .to_vec();
    scoped.extend_from_slice(argv);
    scoped
}

/// Whether `program` is in `PATH`.
pub fn installed(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn the_program_is_not_our_child() {
        let dir = std::env::temp_dir().join(format!("mochi-detach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("parent");
        // The program writes its parent pid, its session and its own pid,
        // from /proc with shell builtins only, then exits.
        let script = format!(
            "read -r _ _ _ ppid _ sid _ < /proc/$$/stat; echo $ppid $sid $$ > {}",
            out.display()
        );
        spawn_detached(&["sh".into(), "-c".into(), script], Some(&dir)).unwrap();

        let started = Instant::now();
        while !out.exists() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let written = std::fs::read_to_string(&out).unwrap();
        let fields: Vec<&str> = written.split_whitespace().collect();
        let parent: u32 = fields[0].parse().unwrap();
        assert_ne!(parent, std::process::id(), "the program is still our child");
        // It leads its own session, away from mochid's signals.
        assert_eq!(fields[1], fields[2]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reports_a_missing_program() {
        let error = spawn_detached(&["mochi-no-such-program".into()], None).unwrap_err();
        assert!(
            error.starts_with("cannot start mochi-no-such-program"),
            "{error}"
        );
    }

    #[test]
    fn scopes_only_with_systemd_run() {
        let argv = vec!["satty".to_owned()];
        let scoped = in_app_scope(&argv);
        if installed("systemd-run") {
            assert_eq!(scoped[0], "systemd-run");
            assert_eq!(scoped.last().map(String::as_str), Some("satty"));
        } else {
            assert_eq!(scoped, argv);
        }
    }
}

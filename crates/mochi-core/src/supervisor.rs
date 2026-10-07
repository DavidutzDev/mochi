//! Runs Quickshell as a child process and restarts it when it exits.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

/// The version pinned in flake.lock. Bump both together.
pub const QUICKSHELL_VERSION: &str = "0.3.1";

/// Quickshell is before 1.0, so a minor release may break its QML API. Any
/// patch release of the pinned minor version is accepted.
const QUICKSHELL_SERIES: (u32, u32) = (0, 3);

/// Giving up after this many exits inside `CRASH_WINDOW`.
const MAX_CRASHES: usize = 5;
const CRASH_WINDOW: Duration = Duration::from_secs(30);
const FIRST_BACKOFF: Duration = Duration::from_millis(250);
const MAX_BACKOFF: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct UiCommand {
    pub program: OsString,
    pub shell_dir: PathBuf,
    pub socket: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    /// A new Quickshell process is running. It should say hello soon.
    Started { pid: u32 },
    /// Quickshell exited too often; the supervisor stopped restarting it.
    GaveUp,
}

#[derive(Debug)]
pub struct Supervisor {
    pid: Arc<Mutex<Option<u32>>>,
    stopping: Arc<AtomicBool>,
    /// Set by [`Supervisor::reload`]: the next exit is asked for, not a
    /// crash.
    reloading: Arc<AtomicBool>,
}

impl Supervisor {
    /// Starts Quickshell and keeps it running.
    ///
    /// The process is spawned from a dedicated thread that lives as long as
    /// the supervisor: `PR_SET_PDEATHSIG` fires when the spawning *thread*
    /// exits, so a tokio worker thread would kill Quickshell at random.
    pub fn spawn(command: UiCommand, events: UnboundedSender<UiEvent>) -> io::Result<Self> {
        let pid = Arc::new(Mutex::new(None));
        let stopping = Arc::new(AtomicBool::new(false));
        let reloading = Arc::new(AtomicBool::new(false));
        let supervisor = Self {
            pid: Arc::clone(&pid),
            stopping: Arc::clone(&stopping),
            reloading: Arc::clone(&reloading),
        };

        thread::Builder::new()
            .name("quickshell".into())
            .spawn(move || supervise(&command, &pid, &stopping, &reloading, &events))?;
        Ok(supervisor)
    }

    /// Kills Quickshell so the supervisor starts a fresh one. SIGKILL,
    /// because this is for a process that stopped responding, and a hung or
    /// stopped process never handles SIGTERM.
    pub fn restart(&self) {
        self.signal(libc::SIGKILL);
    }

    /// Stops Quickshell and starts a fresh one at once, which reads the shell
    /// directory again: Quickshell only finds a directory's QML types when it
    /// starts, so views of a module added since then can't use each other.
    pub fn reload(&self) {
        self.reloading.store(true, Ordering::SeqCst);
        self.signal(libc::SIGTERM);
    }

    /// Stops Quickshell for good.
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.signal(libc::SIGTERM);
    }

    fn signal(&self, signal: libc::c_int) {
        if let Some(pid) = *self.pid.lock().expect("pid lock") {
            // SAFETY: kill has no memory-safety preconditions. The pid is our
            // child and is cleared as soon as it has been reaped.
            unsafe { libc::kill(pid as libc::pid_t, signal) };
        }
    }
}

/// Returns an error unless `program --version` reports a version in
/// `QUICKSHELL_SERIES`.
pub fn check_version(program: &OsString) -> io::Result<()> {
    let output = Command::new(program)
        .arg("--version")
        .output()
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => io::Error::new(
                error.kind(),
                format!("{} is not in PATH", program.to_string_lossy()),
            ),
            _ => error,
        })?;

    let text = String::from_utf8_lossy(&output.stdout);
    let found = text.split_whitespace().nth(1).unwrap_or("unknown");
    if !compatible(found) {
        let (major, minor) = QUICKSHELL_SERIES;
        return Err(io::Error::other(format!(
            "quickshell {found} is installed, mochi needs {major}.{minor}.x \
             and is tested against {QUICKSHELL_VERSION}"
        )));
    }
    Ok(())
}

/// Whether a version such as `0.3.0` or `0.3.1-git` is in `QUICKSHELL_SERIES`.
fn compatible(version: &str) -> bool {
    let mut parts = version.split('.');
    let mut number = || parts.next().and_then(|part| part.parse::<u32>().ok());
    matches!((number(), number()), (Some(major), Some(minor)) if (major, minor) == QUICKSHELL_SERIES)
}

fn supervise(
    command: &UiCommand,
    pid: &Mutex<Option<u32>>,
    stopping: &AtomicBool,
    reloading: &AtomicBool,
    events: &UnboundedSender<UiEvent>,
) {
    let mut exits = VecDeque::new();

    while !stopping.load(Ordering::SeqCst) {
        match start(command) {
            Ok(mut child) => {
                let id = child.id();
                *pid.lock().expect("pid lock") = Some(id);
                tracing::info!(pid = id, "started quickshell");
                if events.send(UiEvent::Started { pid: id }).is_err() {
                    let _ = child.kill();
                    return;
                }

                forward_logs(child.stdout.take());
                forward_logs(child.stderr.take());

                let status = child.wait();
                *pid.lock().expect("pid lock") = None;
                match status {
                    Ok(status) if stopping.load(Ordering::SeqCst) => {
                        tracing::info!(%status, "quickshell stopped");
                        return;
                    }
                    Ok(_) if reloading.swap(false, Ordering::SeqCst) => {
                        tracing::info!("restarting quickshell for new views");
                        continue;
                    }
                    Ok(status) => tracing::warn!(%status, "quickshell exited"),
                    Err(error) => tracing::error!(%error, "lost track of quickshell"),
                }
            }
            Err(error) => tracing::error!(%error, "could not start quickshell"),
        }

        let now = Instant::now();
        exits.push_back(now);
        while exits
            .front()
            .is_some_and(|exit| now.duration_since(*exit) > CRASH_WINDOW)
        {
            exits.pop_front();
        }
        if exits.len() >= MAX_CRASHES {
            tracing::error!(
                "quickshell exited {MAX_CRASHES} times in {}s, giving up",
                CRASH_WINDOW.as_secs()
            );
            let _ = events.send(UiEvent::GaveUp);
            return;
        }

        let backoff = FIRST_BACKOFF * 2u32.pow(exits.len() as u32 - 1);
        thread::sleep(backoff.min(MAX_BACKOFF));
    }
}

fn start(command: &UiCommand) -> io::Result<Child> {
    let mut process = Command::new(&command.program);
    process
        .arg("--path")
        .arg(&command.shell_dir)
        .arg("--no-color")
        .env(mochi_protocol::SOCKET_ENV, &command.socket)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // SAFETY: prctl is async-signal-safe, so it may run between fork and exec.
    unsafe {
        process.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    process.spawn()
}

/// Sends Quickshell's log lines to tracing, keeping their level.
fn forward_logs(stream: Option<impl Read + Send + 'static>) {
    let Some(stream) = stream else { return };
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let line = line.trim();
            match line.split_whitespace().next() {
                Some("WARN") => tracing::warn!(target: "quickshell", "{line}"),
                Some("ERROR" | "FATAL" | "CRIT") => tracing::error!(target: "quickshell", "{line}"),
                Some("DEBUG") => tracing::debug!(target: "quickshell", "{line}"),
                _ => tracing::info!(target: "quickshell", "{line}"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_any_patch_of_the_pinned_minor_version() {
        assert!(compatible(QUICKSHELL_VERSION));
        assert!(compatible("0.3.0"));
        assert!(compatible("0.3.7-git"));
    }

    #[test]
    fn rejects_other_minor_versions_and_garbage() {
        assert!(!compatible("0.2.1"));
        assert!(!compatible("0.4.0"));
        assert!(!compatible("1.3.0"));
        assert!(!compatible("0"));
        assert!(!compatible("unknown"));
        assert!(!compatible(""));
    }
}

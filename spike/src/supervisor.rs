//! Runs Quickshell as a child process and restarts it when it exits.

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::Event;

/// The version pinned in flake.lock. Bump both together.
const QUICKSHELL_VERSION: &str = "0.3.1";

/// Giving up after this many exits inside `CRASH_WINDOW`.
const MAX_CRASHES: usize = 5;
const CRASH_WINDOW: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(5);

pub struct Supervisor {
    pid: Arc<Mutex<Option<u32>>>,
}

impl Supervisor {
    /// Kills the running Quickshell. Only used when it stopped responding, so
    /// SIGKILL: a hung or stopped process never handles SIGTERM. The
    /// supervisor thread starts a new one, and the kill counts towards the
    /// crash limit.
    pub fn restart(&self) {
        if let Some(pid) = *self.pid.lock().unwrap() {
            // SAFETY: kill has no memory-safety preconditions. The pid is our
            // own child and is cleared once it has been reaped.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        }
    }
}

pub fn spawn(shell_dir: PathBuf, socket: PathBuf, events: Sender<Event>) -> io::Result<Supervisor> {
    check_version()?;

    let pid = Arc::new(Mutex::new(None));
    let supervisor = Supervisor {
        pid: Arc::clone(&pid),
    };

    // PR_SET_PDEATHSIG fires when the thread that spawned the child exits,
    // not the process. This thread runs for the daemon's whole lifetime, so
    // the signal fires exactly when mochid dies.
    thread::Builder::new()
        .name("supervisor".into())
        .spawn(move || supervise(&shell_dir, &socket, &pid, &events))?;

    Ok(supervisor)
}

fn check_version() -> io::Result<()> {
    let output = Command::new("quickshell")
        .arg("--version")
        .output()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                io::Error::new(error.kind(), "quickshell is not in PATH")
            } else {
                error
            }
        })?;
    let text = String::from_utf8_lossy(&output.stdout);
    let found = text.split_whitespace().nth(1).unwrap_or("unknown");

    if found != QUICKSHELL_VERSION {
        return Err(io::Error::other(format!(
            "quickshell {found} is installed, mochi is tested against {QUICKSHELL_VERSION}"
        )));
    }
    Ok(())
}

fn supervise(
    shell_dir: &PathBuf,
    socket: &PathBuf,
    pid: &Mutex<Option<u32>>,
    events: &Sender<Event>,
) {
    let mut exits = VecDeque::new();

    loop {
        match start(shell_dir, socket) {
            Ok(mut child) => {
                *pid.lock().unwrap() = Some(child.id());
                eprintln!("supervisor: started quickshell (pid {})", child.id());
                if events.send(Event::UiStarted).is_err() {
                    return;
                }

                forward("quickshell", child.stdout.take());
                forward("quickshell", child.stderr.take());

                let status = child.wait();
                *pid.lock().unwrap() = None;
                match status {
                    Ok(status) => eprintln!("supervisor: quickshell exited ({status})"),
                    Err(error) => eprintln!("supervisor: lost quickshell: {error}"),
                }
            }
            Err(error) => eprintln!("supervisor: could not start quickshell: {error}"),
        }

        let now = Instant::now();
        exits.push_back(now);
        while exits.front().is_some_and(|exit| now - *exit > CRASH_WINDOW) {
            exits.pop_front();
        }
        if exits.len() >= MAX_CRASHES {
            eprintln!(
                "supervisor: quickshell exited {MAX_CRASHES} times in {}s, giving up",
                CRASH_WINDOW.as_secs()
            );
            return;
        }

        let backoff = Duration::from_millis(250) * 2u32.pow(exits.len() as u32 - 1);
        thread::sleep(backoff.min(MAX_BACKOFF));
    }
}

fn start(shell_dir: &PathBuf, socket: &PathBuf) -> io::Result<std::process::Child> {
    let mut command = Command::new("quickshell");
    command
        .arg("--path")
        .arg(shell_dir)
        .arg("--no-color")
        .env("MOCHI_SOCKET", socket)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // SAFETY: prctl is async-signal-safe, so it is allowed between fork and
    // exec.
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    command.spawn()
}

/// Prints every line of a child's output with a prefix. The real daemon sends
/// these to tracing.
fn forward(prefix: &'static str, stream: Option<impl Read + Send + 'static>) {
    let Some(stream) = stream else { return };
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            eprintln!("[{prefix}] {line}");
        }
    });
}

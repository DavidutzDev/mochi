//! One shell per Wayland session. A daemon holds a lock on
//! `$XDG_RUNTIME_DIR/mochi/session-<display>.lock` while it runs, so a
//! second one never draws a second island over the first, whatever
//! `--runtime-dir` says. The file names the holder, so `mochid --dev` can
//! ask it to step aside: the holder stops, waits for the dev daemon to end,
//! then starts again by itself.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Seek, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::unix::AsyncFd;

/// How long a daemon asked to step aside gets to stop.
const STEP_ASIDE: Duration = Duration::from_secs(10);

/// The daemon running the session, as its lock file says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Holder {
    pub pid: u32,
    pub socket: PathBuf,
    pub version: String,
    #[serde(default)]
    pub dev: bool,
}

/// The session's lock, held until dropped.
#[derive(Debug)]
pub struct Session {
    _file: File,
}

/// The lock for the Wayland display in `WAYLAND_DISPLAY`, in `runtime`
/// (`$XDG_RUNTIME_DIR/mochi`, before `--runtime-dir`).
pub fn lock_path(runtime: &Path, display: Option<&str>) -> PathBuf {
    let display = display
        .and_then(|display| Path::new(display).file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("none");
    runtime.join(format!("session-{display}.lock"))
}

impl Session {
    /// Takes the lock and writes `holder` into it, or says who holds it.
    pub fn try_acquire(path: &Path, holder: &Holder) -> io::Result<Result<Self, Option<Holder>>> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        // SAFETY: flock has no memory-safety preconditions; the fd is open.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            if error.kind() != ErrorKind::WouldBlock {
                return Err(error);
            }
            let mut text = String::new();
            file.read_to_string(&mut text)?;
            return Ok(Err(serde_json::from_str(&text).ok()));
        }
        file.set_len(0)?;
        file.rewind()?;
        file.write_all(serde_json::to_string(holder)?.as_bytes())?;
        Ok(Ok(Self { _file: file }))
    }

    /// Takes the lock, asking its holder to step aside first when `take`
    /// says so. Fails with a message naming the holder otherwise.
    /// Also returns the daemon that stepped aside, if one did.
    pub async fn acquire(
        path: &Path,
        holder: &Holder,
        take: bool,
    ) -> anyhow::Result<(Self, Option<Holder>)> {
        let other = match Self::try_acquire(path, holder)? {
            Ok(session) => return Ok((session, None)),
            Err(other) => other,
        };
        let named = match &other {
            Some(other) => format!(
                "mochid {} (pid {}{})",
                other.version,
                other.pid,
                if other.dev { ", --dev" } else { "" }
            ),
            None => "another mochid".to_owned(),
        };
        let Some(other) = other.filter(|_| take) else {
            anyhow::bail!(
                "{named} already runs this session: stop it, or run `mochid --dev`, which takes over until it stops"
            );
        };
        step_aside(&other.socket, holder.pid)
            .await
            .map_err(|error| anyhow::anyhow!("{named} didn't step aside: {error}"))?;
        tracing::info!(pid = other.pid, "the running shell stepped aside");
        let deadline = Instant::now() + STEP_ASIDE;
        loop {
            if let Ok(session) = Self::try_acquire(path, holder)? {
                return Ok((session, Some(other)));
            }
            if Instant::now() > deadline {
                anyhow::bail!("{named} said it would step aside, but still runs");
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

/// Binds `socket` once the daemon on it has stopped.
pub async fn bind_when_free(socket: &Path) -> anyhow::Result<tokio::net::UnixListener> {
    let deadline = Instant::now() + STEP_ASIDE;
    loop {
        match crate::ipc::bind(socket).await {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == ErrorKind::AddrInUse && Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

/// Asks the daemon on `socket` to step aside for process `pid`.
pub async fn step_aside(socket: &Path, pid: u32) -> anyhow::Result<()> {
    use mochi_protocol::{API, ClientMessage, DaemonMessage, Role};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let stream = tokio::net::UnixStream::connect(socket).await?;
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    for message in [
        ClientMessage::Hello {
            api: API,
            role: Role::Ctl,
        },
        ClientMessage::StepAside { pid },
    ] {
        let mut line = serde_json::to_string(&message)?;
        line.push('\n');
        writer.write_all(line.as_bytes()).await?;
        let answer = match tokio::time::timeout(STEP_ASIDE, lines.next_line()).await? {
            Ok(Some(answer)) => answer,
            // It may close the connection as it stops, before the answer goes.
            _ if matches!(message, ClientMessage::StepAside { .. }) => return Ok(()),
            Ok(None) => anyhow::bail!("it closed the connection"),
            Err(error) => return Err(error.into()),
        };
        match serde_json::from_str::<DaemonMessage>(&answer)? {
            DaemonMessage::Hello { .. } | DaemonMessage::Ok => {}
            DaemonMessage::Error { message, .. } => {
                tracing::debug!(%message, "the running daemon's answer");
                anyhow::bail!(
                    "it's a version from before daemons could step aside: stop it first, for example with `systemctl --user stop mochid`"
                )
            }
            other => anyhow::bail!("unexpected answer {other:?}"),
        }
    }
    Ok(())
}

/// What a daemon that stepped aside does once the other one is gone.
pub enum Parked {
    /// Start again.
    Resume,
    /// It was told to stop while it waited.
    Stop,
}

/// Waits for process `pid` to end, or for SIGTERM or SIGINT.
pub async fn park(pid: u32) -> anyhow::Result<Parked> {
    tracing::info!(pid, "stepped aside; back when that process stops");
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let Some(fd) = pidfd(pid)? else {
        return Ok(Parked::Resume);
    };
    let fd = AsyncFd::new(fd)?;
    tokio::select! {
        ready = fd.readable() => {
            drop(ready?);
            Ok(Parked::Resume)
        }
        _ = tokio::signal::ctrl_c() => Ok(Parked::Stop),
        _ = terminate.recv() => Ok(Parked::Stop),
    }
}

/// A pidfd for `pid`, readable once it ends; `None` when it already has.
fn pidfd(pid: u32) -> io::Result<Option<OwnedFd>> {
    // SAFETY: pidfd_open takes a pid and flags and returns a new fd or -1.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0) };
    if fd < 0 {
        let error = io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(libc::ESRCH) => Ok(None),
            _ => Err(error),
        };
    }
    // SAFETY: the syscall returned a new fd that nothing else owns.
    Ok(Some(unsafe { OwnedFd::from_raw_fd(fd as libc::c_int) }))
}

/// Starts this daemon again, with the same arguments and environment, in
/// the same process, so a service manager keeps following it.
pub fn restart() -> io::Error {
    let mut args = std::env::args_os();
    let first = args.next().unwrap_or_default();
    std::process::Command::new("/proc/self/exe")
        .arg0(first)
        .args(args)
        .exec()
}

/// What the dev bubble shows: the revision of the source tree this daemon
/// runs from, and the daemon it took over from.
pub fn dev_payload(took_over: Option<&Holder>) -> serde_json::Value {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(source)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    let revision = git(&["describe", "--always", "--dirty=*", "--abbrev=7"]);
    let short = git(&["rev-parse", "--short=7", "HEAD"]).map(|short| match revision.as_deref() {
        Some(revision) if revision.ends_with('*') => format!("{short}*"),
        _ => short,
    });
    serde_json::json!({
        "revision": revision,
        "short": short,
        "branch": git(&["branch", "--show-current"]),
        "build": if cfg!(debug_assertions) { "debug" } else { "release" },
        "pid": std::process::id(),
        "took_over": took_over.map(|holder| serde_json::json!({
            "pid": holder.pid,
            "version": holder.version,
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holder(pid: u32) -> Holder {
        Holder {
            pid,
            socket: PathBuf::from("/run/mochi.sock"),
            version: "0.1.2".into(),
            dev: false,
        }
    }

    #[test]
    fn the_lock_is_named_after_the_display() {
        let dir = Path::new("/run/user/1000/mochi");
        assert_eq!(
            lock_path(dir, Some("wayland-1")),
            dir.join("session-wayland-1.lock")
        );
        assert_eq!(
            lock_path(dir, Some("/run/user/1000/wayland-1")),
            dir.join("session-wayland-1.lock")
        );
        assert_eq!(lock_path(dir, None), dir.join("session-none.lock"));
    }

    #[test]
    fn a_second_holder_learns_who_holds_it() {
        let dir = std::env::temp_dir().join(format!("mochi-session-{}", std::process::id()));
        let path = dir.join("session-test.lock");
        let first = Session::try_acquire(&path, &holder(1)).unwrap().unwrap();
        assert_eq!(
            Session::try_acquire(&path, &holder(2))
                .unwrap()
                .unwrap_err(),
            Some(holder(1))
        );
        drop(first);
        assert!(Session::try_acquire(&path, &holder(2)).unwrap().is_ok());
        fs::remove_dir_all(dir).unwrap();
    }
}

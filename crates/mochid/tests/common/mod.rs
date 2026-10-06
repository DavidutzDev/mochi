//! Runs a real `mochid` in a scratch directory and talks to it over its
//! socket, playing the UI or the CLI.

#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use mochi_protocol::{API, Activity, Bubble, ClientMessage, DaemonMessage, Role};

const BINARY: &str = env!("CARGO_BIN_EXE_mochid");
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// Stands in for Quickshell: answers `--version` like the pinned release,
/// records each start, then waits until killed. It never connects, so tests
/// can play the UI themselves.
const FAKE_QUICKSHELL: &str = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "Quickshell 0.3.1 (revision test, distributed by tests)"
    exit 0
fi
echo "$$ $MOCHI_SOCKET" >> "$(dirname "$0")/starts"
exec sleep 3600
"#;

pub struct Daemon {
    pub dir: PathBuf,
    pub socket: PathBuf,
    child: Child,
}

impl Daemon {
    /// Starts `mochid` with a fake Quickshell and waits for its socket.
    pub fn start(name: &str, modules: &str) -> Self {
        Self::start_with(name, modules, None)
    }

    /// Like `start`, with a real Quickshell when `quickshell` is given.
    pub fn start_with(name: &str, modules: &str, quickshell: Option<&str>) -> Self {
        Self::start_prepared(name, modules, quickshell, |_, _| {})
    }

    /// Like `start_with`, with `prepare` setting up the scratch directory
    /// and the command first: config files, plugins, environment.
    pub fn start_prepared(
        name: &str,
        modules: &str,
        quickshell: Option<&str>,
        prepare: impl FnOnce(&Path, &mut Command),
    ) -> Self {
        let dir = std::env::temp_dir().join(format!("mochid-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("run")).unwrap();

        let quickshell_is_real = quickshell.is_some();
        let quickshell = match quickshell {
            Some(program) => PathBuf::from(program),
            None => {
                let fake = dir.join("quickshell");
                fs::write(&fake, FAKE_QUICKSHELL).unwrap();
                fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
                fake
            }
        };

        let log = fs::File::create(dir.join("mochid.log")).unwrap();
        let mut command = Command::new(BINARY);
        command
            .args(["--modules", modules, "--quickshell"])
            .arg(&quickshell)
            // Private runtime, config, data and state dirs, so a daemon
            // already running in the session and the user's files don't
            // matter, and the tests never write to them.
            .env("XDG_RUNTIME_DIR", dir.join("run"))
            .env("XDG_CONFIG_HOME", dir.join("config"))
            .env("XDG_DATA_HOME", dir.join("data"))
            .env("XDG_STATE_HOME", dir.join("state"))
            .env("MOCHI_LOG", "debug")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        if quickshell_is_real {
            // The private XDG_RUNTIME_DIR hides the compositor's socket, so
            // point Quickshell at it directly.
            let runtime = std::env::var_os("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is set");
            let display = std::env::var_os("WAYLAND_DISPLAY").expect("a Wayland session");
            command.env("WAYLAND_DISPLAY", Path::new(&runtime).join(display));
        }
        prepare(&dir, &mut command);
        let child = command.spawn().unwrap();

        let socket = dir.join("run/mochi/mochi.sock");
        let daemon = Self { dir, socket, child };
        daemon.wait_for(|| UnixStream::connect(&daemon.socket).is_ok(), "the socket");
        daemon
    }

    pub fn client(&self, role: Role) -> Client {
        Client::connect(&self.socket, role)
    }

    /// The pid and `MOCHI_SOCKET` of every fake Quickshell started so far.
    pub fn quickshell_starts(&self) -> Vec<(u32, String)> {
        fs::read_to_string(self.dir.join("starts"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                let (pid, socket) = line.split_once(' ').unwrap();
                (pid.parse().unwrap(), socket.to_owned())
            })
            .collect()
    }

    pub fn log(&self) -> String {
        fs::read_to_string(self.dir.join("mochid.log")).unwrap_or_default()
    }

    /// Sends SIGTERM and waits for the daemon to exit.
    pub fn terminate(&mut self) -> ExitStatus {
        // SAFETY: kill has no memory-safety preconditions, and the pid is our
        // child, which has not been reaped.
        unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM) };
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "mochid ignored SIGTERM\n{}",
                self.log()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn wait_for(&self, done: impl FnMut() -> bool, what: &str) {
        self.wait_long(done, what, TIMEOUT);
    }

    pub fn wait_long(&self, mut done: impl FnMut() -> bool, what: &str, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while !done() {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}\n{}",
                self.log()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Client {
    /// Connects and completes the handshake.
    pub fn connect(socket: &Path, role: Role) -> Self {
        let mut client = Self::raw(socket);
        client.send(&ClientMessage::Hello { api: API, role });
        assert!(matches!(
            client.recv(),
            DaemonMessage::Hello { api: API, .. }
        ));
        client
    }

    /// Connects without saying hello.
    pub fn raw(socket: &Path) -> Self {
        let stream = UnixStream::connect(socket).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        Self {
            reader: BufReader::new(stream.try_clone().unwrap()),
            writer: stream,
        }
    }

    pub fn send(&mut self, message: &ClientMessage) {
        let line = mochi_protocol::encode(message).unwrap();
        self.writer.write_all(&line).unwrap();
    }

    pub fn send_line(&mut self, line: &str) {
        self.writer.write_all(line.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
    }

    pub fn recv(&mut self) -> DaemonMessage {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => panic!("the daemon closed the connection"),
            Ok(_) => mochi_protocol::decode(&line).unwrap(),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                panic!("no message within {TIMEOUT:?}")
            }
            Err(error) => panic!("read failed: {error}"),
        }
    }

    /// Whether the daemon closed the connection, waiting up to `TIMEOUT`.
    pub fn closed(&mut self) -> bool {
        let mut line = String::new();
        matches!(self.reader.read_line(&mut line), Ok(0))
    }

    /// Reads until the next `present`, skipping other messages.
    pub fn next_present(&mut self) -> Option<Activity> {
        loop {
            if let DaemonMessage::Present { activity, .. } = self.recv() {
                return activity;
            }
        }
    }

    /// Waits until the island shows `view` of `module`, skipping whatever
    /// comes before: right after `hello` the island may still be empty,
    /// with the module's first activity on its way.
    pub fn wait_for_view(&mut self, module: &str, view: &str) -> Activity {
        loop {
            if let Some(activity) = self.next_present()
                && activity.module == module
                && activity.view == view
            {
                return activity;
            }
        }
    }

    /// The next bubbles snapshot, skipping anything else.
    pub fn next_bubbles(&mut self) -> Vec<Bubble> {
        loop {
            if let DaemonMessage::Bubbles { bubbles, .. } = self.recv() {
                return bubbles;
            }
        }
    }

    /// Runs a command and returns the daemon's answer.
    pub fn command(&mut self, module: &str, action: &str, args: &[&str]) -> DaemonMessage {
        self.send(&ClientMessage::Command {
            module: module.into(),
            action: action.into(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        });
        self.recv()
    }
}

/// `(module, view)` of an activity, for short assertions.
pub fn shown(activity: &Option<Activity>) -> Option<(&str, &str)> {
    activity
        .as_ref()
        .map(|activity| (activity.module.as_str(), activity.view.as_str()))
}

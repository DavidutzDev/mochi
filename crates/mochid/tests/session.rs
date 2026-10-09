//! One shell per session: a second daemon refuses to start, and
//! `mochid --dev` takes over from the running one, which starts again once
//! the dev daemon stops.

mod common;

use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Client, Daemon, TIMEOUT};
use mochi_protocol::Role;

/// A second daemon in the first one's session, with a runtime dir of its own
/// as `--runtime-dir` gives.
fn second(first: &Daemon, name: &str, dev: bool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mochid"));
    command
        .args(["--modules", "idle", "--quickshell"])
        .arg(first.dir.join("quickshell"))
        .arg("--runtime-dir")
        .arg(first.dir.join(name))
        .env("XDG_RUNTIME_DIR", first.dir.join("run"))
        .env("XDG_CONFIG_HOME", first.dir.join("config"))
        .env("XDG_DATA_HOME", first.dir.join("data"))
        .env("XDG_STATE_HOME", first.dir.join("state"))
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    if dev {
        command.arg("--dev");
    }
    command
}

fn wait(mut done: impl FnMut() -> bool, what: &str) {
    let deadline = Instant::now() + TIMEOUT * 2;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_second_daemon_leaves_the_session_alone() {
    let first = Daemon::start("session-second", "idle");
    let output = second(&first, "second", false).output().unwrap();
    assert!(!output.status.success());
    let log = String::from_utf8_lossy(&output.stderr);
    assert!(log.contains("already runs this session"), "{log}");
    assert!(UnixStream::connect(&first.socket).is_ok());
}

#[test]
fn a_dev_daemon_takes_over_until_it_stops() {
    let mut first = Daemon::start("session-dev", "idle");
    let dev_socket = first.dir.join("dev/mochi.sock");
    let mut dev = second(&first, "dev", true)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // The first one stops its shell and waits, without exiting.
    wait(
        || UnixStream::connect(&dev_socket).is_ok(),
        "the dev socket",
    );
    assert!(UnixStream::connect(&first.socket).is_err());
    assert!(first.running(), "the first daemon exited\n{}", first.log());
    let first_shells = |first: &Daemon| {
        first
            .quickshell_starts()
            .iter()
            .filter(|(_, socket)| *socket == first.socket.display().to_string())
            .count()
    };
    assert_eq!(first_shells(&first), 1);

    // The dev daemon says what it is, and whom it took over from.
    let mut ui = Client::connect(&dev_socket, Role::Ui);
    let bubbles = ui.next_bubbles();
    let bubble = bubbles
        .iter()
        .find(|bubble| bubble.module == "mochi" && bubble.view == "DevBubble")
        .expect("the dev bubble");
    assert_eq!(bubble.payload["took_over"]["pid"], first.pid());

    // Once it stops, the first one starts again, shell and all.
    // SAFETY: kill has no memory-safety preconditions; dev is our child.
    unsafe { libc::kill(dev.id() as libc::pid_t, libc::SIGTERM) };
    dev.wait().unwrap();
    wait(
        || UnixStream::connect(&first.socket).is_ok(),
        "the first socket again",
    );
    assert!(first.running());
    wait(
        || first_shells(&first) == 2,
        "the first daemon's shell again",
    );
    first.terminate();
}

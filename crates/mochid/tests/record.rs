//! Runs `mochid` with the real Quickshell in the current Wayland session,
//! walks the island through the arbiter's rules and records it.
//!
//! It draws on your screen, so it only runs on request, from the dev shell:
//!
//!     cargo test -p mochid --test record -- --ignored --nocapture
//!
//! The video goes to `/tmp/mochi-recordings/demo.mp4`, outside the
//! repository; set `MOCHI_RECORD_OUTPUT` to put it elsewhere. Recordings are
//! never committed. The recorded area is the top center of the focused
//! monitor, read from `hyprctl`. On other compositors, set it yourself:
//!
//!     MOCHI_RECORD_GEOMETRY="2560,0 640x640" cargo test -p mochid --test record -- --ignored

#![allow(clippy::print_stderr)]

mod common;

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use common::{Client, Daemon, shown};
use mochi_protocol::{Activity, ClientMessage, DaemonMessage, EventKind, Role};

/// Width and height of the recorded area. Even numbers, for the encoder.
const RECORD_SIZE: (i64, i64) = (640, 640);

#[test]
#[ignore = "draws on the screen and records it; run with --ignored"]
fn record_the_island() {
    let daemon = Daemon::start_with("record", "idle,demo", Some("quickshell"));
    let mut ctl = daemon.client(Role::Ctl);
    daemon.wait_for(|| ui_connected(&mut ctl), "Quickshell to connect");

    // A second UI connection sees every `present` the real one gets, and can
    // send clicks without moving the pointer.
    let mut watcher = daemon.client(Role::Ui);
    assert_eq!(shown(&watcher.next_present()), Some(("idle", "Pill")));

    let output = recording_path();
    let geometry = record_geometry();
    eprintln!("recording {geometry} to {}", output.display());
    let recorder = Recorder::start(&geometry, &output);
    // wf-recorder needs a moment before the first frame lands.
    hold(800);

    hold(1200);
    let w = &mut watcher;

    // The card has a 4s timeout. Keeping it hovered, as a pointer resting on
    // the island would, pauses that timer, so it only ends when told to.
    // Hovering resets when another activity interrupts it, like the real UI.
    run(
        &mut ctl,
        &[
            "show",
            "Card",
            "A normal-priority card, hovered so it stays.",
        ],
    );
    let card = step(w, "show a card", ("demo", "Card"), 0);
    event(w, &card, EventKind::HoverEnter);
    hold(1800);

    run(
        &mut ctl,
        &["alert", "Small", "High priority: interrupts the card"],
    );
    let alert = step(w, "an alert interrupts it", ("demo", "Small"), 1500);

    event(w, &alert, EventKind::Dismiss);
    let back = step(
        w,
        "closing the alert brings the card back",
        ("demo", "Card"),
        0,
    );
    assert_eq!(back.id, card.id);
    event(w, &card, EventKind::HoverEnter);
    hold(1200);

    event(w, &card, EventKind::Click);
    step(
        w,
        "a click expands the card",
        ("demo", "CardExpanded"),
        2000,
    );
    event(w, &card, EventKind::Click);
    step(w, "another click collapses it", ("demo", "Card"), 1000);

    for level in ["30", "70", "100"] {
        run(&mut ctl, &["volume", level]);
        step(
            w,
            "volume replaces itself in place",
            ("demo", "Volume"),
            600,
        );
    }
    step(
        w,
        "the card resumes when the volume times out",
        ("demo", "Card"),
        0,
    );
    event(w, &card, EventKind::HoverEnter);
    hold(1200);

    run(
        &mut ctl,
        &["stack", "Wide", "Same priority, stacked on top"],
    );
    step(w, "a stacked activity interrupts", ("demo", "Wide"), 1800);

    run(&mut ctl, &["alert", "Big"]);
    step(w, "the island grows to 520px", ("demo", "Big"), 2000);

    run(&mut ctl, &["clear"]);
    step(w, "clear returns straight to idle", ("idle", "Pill"), 1500);

    recorder.stop();
    let size = std::fs::metadata(&output)
        .expect("the recording exists")
        .len();
    assert!(size > 10_000, "the recording is only {size} bytes");
    eprintln!("saved {} ({} KiB)", output.display(), size / 1024);
}

/// Waits for the next `present`, checks it shows `expected`, and leaves it on
/// screen for `wait_ms`.
fn step(watcher: &mut Client, name: &str, expected: (&str, &str), wait_ms: u64) -> Activity {
    eprintln!("step: {name}");
    let activity = watcher.next_present().expect("an activity");
    assert_eq!(
        (activity.module.as_str(), activity.view.as_str()),
        expected,
        "{name}"
    );
    hold(wait_ms);
    activity
}

fn run(ctl: &mut Client, words: &[&str]) {
    let (action, args) = words.split_first().unwrap();
    assert_eq!(
        ctl.command("demo", action, args),
        DaemonMessage::Ok,
        "{words:?}"
    );
}

fn event(ui: &mut Client, activity: &Activity, kind: EventKind) {
    ui.send(&ClientMessage::Event {
        activity: activity.id,
        kind,
        output: None,
        click: None,
    });
}

fn ui_connected(ctl: &mut Client) -> bool {
    ctl.send(&ClientMessage::Status);
    matches!(ctl.recv(), DaemonMessage::Status { status } if status.ui_connected)
}

fn hold(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

/// wf-recorder, stopped with SIGINT so it writes a playable file.
struct Recorder(Child);

impl Recorder {
    fn start(geometry: &str, output: &PathBuf) -> Self {
        let child = Command::new("wf-recorder")
            .args(["--geometry", geometry, "--overwrite", "--file"])
            .arg(output)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("wf-recorder starts (run the test inside `nix develop`)");
        Self(child)
    }

    fn stop(mut self) {
        // SAFETY: kill has no memory-safety preconditions, and the pid is our
        // child, which has not been reaped.
        unsafe { libc::kill(self.0.id() as libc::pid_t, libc::SIGINT) };
        let _ = self.0.wait();
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if let Ok(None) = self.0.try_wait() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn record_geometry() -> String {
    if let Ok(geometry) = std::env::var("MOCHI_RECORD_GEOMETRY") {
        return geometry;
    }
    let output = Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
        .expect("hyprctl runs; on other compositors set MOCHI_RECORD_GEOMETRY");
    let monitors: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("hyprctl prints JSON");
    let monitor = monitors
        .iter()
        .find(|monitor| monitor["focused"] == true)
        .expect("a monitor is focused");

    let number = |key: &str| monitor[key].as_i64().expect("monitor geometry is numeric");
    let (width, height) = RECORD_SIZE;
    let x = number("x") + number("width") / 2 - width / 2;
    format!("{x},{} {width}x{height}", number("y"))
}

/// Outside the repository, so a recording never ends up in a commit.
fn recording_path() -> PathBuf {
    let path = std::env::var_os("MOCHI_RECORD_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mochi-recordings/demo.mp4"));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("the recordings directory can be created");
    }
    path
}

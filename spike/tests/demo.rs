//! Runs the spike in the current Wayland session, walks the island through
//! every demo view and records it to `recordings/demo.mp4`.
//!
//! It draws on your screen, so it only runs on request, from the dev shell:
//!
//!     cargo test --test demo -- --ignored --nocapture
//!
//! The recorded area is the top center of the focused monitor, read from
//! `hyprctl`. On other compositors, set it yourself:
//!
//!     MOCHI_RECORD_GEOMETRY="2560,0 640x640" cargo test --test demo -- --ignored

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const BINARY: &str = env!("CARGO_BIN_EXE_mochi-spike");

/// Width and height of the recorded area. Even numbers, for the encoder.
const RECORD_SIZE: (i64, i64) = (640, 640);

const LONG_TEXT: &str = "A longer body makes a taller card. The view only sets its width; \
     the text wraps, the view's implicit height grows, and the island springs to it.";

/// Kills the process when dropped, so a failing assertion never leaves the
/// island or the recorder running.
struct Process {
    name: &'static str,
    child: Child,
}

impl Process {
    /// Stops the process with SIGINT and waits for it. wf-recorder only
    /// writes a playable file when it is interrupted this way.
    fn interrupt(mut self) {
        // SAFETY: kill has no memory-safety preconditions, and the pid
        // belongs to a child we haven't reaped yet.
        unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGINT) };
        let _ = self.child.wait();
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            eprintln!("demo: stopping {}", self.name);
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[test]
#[ignore = "draws on the screen and records it; run with --ignored"]
fn record_demo() {
    assert!(
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        "needs a Wayland session"
    );

    let geometry = record_geometry();
    let output = recording_path();

    let _daemon = start_daemon();

    eprintln!("demo: recording {geometry} to {}", output.display());
    let recorder = start_recorder(&geometry, &output);
    // wf-recorder needs a moment before the first frame lands.
    thread::sleep(Duration::from_millis(800));

    show(&["idle", "show"], 1500);
    show(&["demo", "show", "Small"], 1500);
    show(&["demo", "show", "Wide", "Some song - Some artist"], 1500);
    show(&["demo", "show", "Card"], 2000);
    show(&["demo", "show", "Card", LONG_TEXT], 2000);
    show(&["demo", "show", "Big"], 2000);
    // No explicit return to idle: the 4s timeout does it.
    show(&["demo", "show", "Small"], 5500);

    recorder.interrupt();

    let size = std::fs::metadata(&output)
        .expect("the recording exists")
        .len();
    assert!(size > 10_000, "the recording is only {size} bytes");
    eprintln!("demo: saved {} ({} KiB)", output.display(), size / 1024);
}

/// Starts the spike and waits until Quickshell has connected to it.
fn start_daemon() -> Process {
    let mut child = Command::new(BINARY)
        .arg("run")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the spike binary starts");

    // Keep draining the daemon's log for the whole test. A full pipe would
    // block the daemon on its next log line.
    let stderr = child.stderr.take().expect("stderr is piped");
    let (connected, ui_ready) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            eprintln!("  {line}");
            if line.starts_with("daemon: ui connected") {
                let _ = connected.send(());
            }
        }
    });

    let daemon = Process {
        name: "the spike",
        child,
    };
    ui_ready
        .recv_timeout(Duration::from_secs(15))
        .expect("Quickshell connects to the daemon within 15s");
    daemon
}

fn start_recorder(geometry: &str, output: &PathBuf) -> Process {
    let child = Command::new("wf-recorder")
        .args(["--geometry", geometry, "--overwrite", "--file"])
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("wf-recorder starts (run the test inside `nix develop`)");
    Process {
        name: "wf-recorder",
        child,
    }
}

/// Sends one `ipc` command through the CLI, then leaves the result on screen.
fn show(args: &[&str], hold_ms: u64) {
    eprintln!("demo: ipc {}", args.join(" "));
    let status = Command::new(BINARY)
        .arg("ipc")
        .args(args)
        .status()
        .expect("the ipc client runs");
    assert!(status.success(), "ipc {} failed", args.join(" "));
    thread::sleep(Duration::from_millis(hold_ms));
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

fn recording_path() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("recordings");
    std::fs::create_dir_all(&dir).expect("the recordings directory can be created");
    dir.join("demo.mp4")
}

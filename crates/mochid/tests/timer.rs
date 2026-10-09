//! The focus timer through a real `mochid`: its bubble comes and goes with
//! the timer, a click on it pauses, and a restart carries on from the
//! session directory. The ends of phases are the module's unit tests, which
//! don't wait minutes.

mod common;

use common::Daemon;
use mochi_protocol::{Bubble, ClientMessage, DaemonMessage, ErrorCode, Role};

fn output(message: DaemonMessage) -> String {
    match message {
        DaemonMessage::Output { output } => output,
        other => panic!("expected output, got {other:?}"),
    }
}

/// The next bubbles snapshot with the timer's bubble, or without one.
fn timer_bubble(ui: &mut common::Client) -> Option<Bubble> {
    ui.next_bubbles()
        .into_iter()
        .find(|bubble| bubble.module == "timer")
}

#[test]
fn the_bubble_follows_the_timer() {
    let daemon = Daemon::start("timer", "idle,timer");
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);
    assert_eq!(timer_bubble(&mut ui), None);
    assert_eq!(
        output(ctl.command("timer", "status", &[])),
        "idle, 0 sessions done"
    );

    assert_eq!(ctl.command("timer", "start", &["10"]), DaemonMessage::Ok);
    let bubble = timer_bubble(&mut ui).expect("a bubble while it runs");
    assert_eq!(bubble.view, "Bubble");
    assert_eq!(bubble.payload["phase"], "focus");
    assert_eq!(bubble.payload["total_ms"], 600_000);
    assert_eq!(bubble.payload["paused"], false);
    assert!(bubble.payload["ends_ms"].is_u64());
    assert!(output(ctl.command("timer", "status", &[])).starts_with("focus "));

    // A click on the bubble pauses it, in place.
    ui.send(&ClientMessage::BubbleClick { bubble: bubble.id });
    let paused = timer_bubble(&mut ui).unwrap();
    assert_eq!(paused.payload["paused"], true);
    assert!(paused.payload["ends_ms"].is_null());
    assert!(output(ctl.command("timer", "status", &[])).ends_with(", paused"));

    let refused = ctl.command("timer", "break", &["0"]);
    assert!(
        matches!(
            refused,
            DaemonMessage::Error {
                code: ErrorCode::ModuleFailed,
                ..
            }
        ),
        "{refused:?}"
    );

    assert_eq!(ctl.command("timer", "stop", &[]), DaemonMessage::Ok);
    assert_eq!(timer_bubble(&mut ui), None);
    let refused = ctl.command("timer", "pause", &[]);
    assert!(
        matches!(
            refused,
            DaemonMessage::Error {
                code: ErrorCode::ModuleFailed,
                ..
            }
        ),
        "{refused:?}"
    );
}

#[test]
fn a_session_that_ran_out_meanwhile_ends_after_a_restart() {
    // What the last run saved: focus that ended long ago, while mochid was
    // down.
    let daemon = Daemon::start_prepared("timer-restart", "idle,timer", None, |dir, _| {
        let session = dir.join("run/mochi/session/timer");
        std::fs::create_dir_all(&session).unwrap();
        let saved = serde_json::json!({
            "countdown": {
                "phase": "focus",
                "total_ms": 1_500_000,
                "ends_ms": 1_000,
                "left_ms": 1_500_000,
            },
            "sessions": 2,
        });
        std::fs::write(session.join("timer.json"), saved.to_string()).unwrap();
    });
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);

    let notice = ui.wait_for_view("timer", "Notice");
    assert_eq!(notice.payload["finished"], "focus");
    assert_eq!(notice.payload["next"], "break");
    assert_eq!(notice.payload["minutes"], 5);
    assert_eq!(notice.payload["sessions"], 3);
    assert_eq!(
        output(ctl.command("timer", "status", &[])),
        "idle, 3 sessions done"
    );

    // Start the break, as the notice offers: the notice goes.
    assert_eq!(ctl.command("timer", "break", &[]), DaemonMessage::Ok);
    ui.wait_for_view("idle", "Pill");
    assert!(output(ctl.command("timer", "status", &[])).starts_with("break 5:00"));
}

/// The bubbles of the custom timers in the next snapshot, in the order
/// they started.
fn custom_bubbles(ui: &mut common::Client) -> Vec<Bubble> {
    let mut bubbles: Vec<Bubble> = ui
        .next_bubbles()
        .into_iter()
        .filter(|bubble| bubble.module == "timer" && bubble.payload["phase"] == "timer")
        .collect();
    bubbles.sort_by_key(|bubble| bubble.payload["id"].as_u64());
    bubbles
}

#[test]
fn custom_timers_run_side_by_side() {
    let daemon = Daemon::start("timer-custom", "idle,timer");
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);
    assert!(custom_bubbles(&mut ui).is_empty());
    assert_eq!(output(ctl.command("timer", "list", &[])), "no timers");

    assert_eq!(
        output(ctl.command("timer", "add", &["10m", "Pizza"])),
        "timer 1"
    );
    assert_eq!(custom_bubbles(&mut ui).len(), 1);
    assert_eq!(output(ctl.command("timer", "add", &["90s"])), "timer 2");
    let bubbles = custom_bubbles(&mut ui);
    assert_eq!(bubbles.len(), 2);
    assert_eq!(bubbles[0].payload["label"], "Pizza");
    assert_eq!(bubbles[1].payload["total_ms"], 90_000);
    assert_eq!(bubbles[1].payload["name"], "Timer 2");
    let list = output(ctl.command("timer", "list", &[]));
    assert!(
        list.starts_with("1 Pizza 10:00 left\n2 Timer 2 1:"),
        "{list}"
    );

    // A click pauses that one only; the focus session isn't touched.
    ui.send(&ClientMessage::BubbleClick {
        bubble: bubbles[1].id,
    });
    let bubbles = custom_bubbles(&mut ui);
    assert_eq!(bubbles[0].payload["paused"], false);
    assert_eq!(bubbles[1].payload["paused"], true);
    assert_eq!(
        output(ctl.command("timer", "status", &[])),
        "idle, 0 sessions done"
    );

    // "+1 min".
    assert_eq!(ctl.command("timer", "extend", &["1"]), DaemonMessage::Ok);
    assert_eq!(custom_bubbles(&mut ui)[0].payload["total_ms"], 660_000);
    let refused = ctl.command("timer", "add", &["Pizza"]);
    assert!(
        matches!(refused, DaemonMessage::Error { .. }),
        "{refused:?}"
    );
    assert_eq!(ctl.command("timer", "stop", &["1"]), DaemonMessage::Ok);
    assert_eq!(custom_bubbles(&mut ui).len(), 1);
    assert_eq!(ctl.command("timer", "stop", &["all"]), DaemonMessage::Ok);
    assert_eq!(custom_bubbles(&mut ui).len(), 0);

    // The launcher's provider answers with a result as a line of JSON, and
    // picking it starts the timer.
    let found = output(ctl.command("timer", "search", &["10m", "Pizza"]));
    let result: serde_json::Value = serde_json::from_str(&found).unwrap();
    assert_eq!(result["title"], "Start a 10 min timer for Pizza");
    let id = result["id"].as_str().unwrap();
    assert_eq!(
        ctl.command("timer", "pick-result", &[id]),
        DaemonMessage::Ok
    );
    assert_eq!(custom_bubbles(&mut ui)[0].payload["label"], "Pizza");
}

#[test]
fn a_timer_that_ran_out_meanwhile_rings_after_a_restart() {
    // A custom timer that ran out while mochid was down, and an alarm
    // played by a script that notes what it got, never the speakers.
    let daemon = Daemon::start_prepared("timer-ring", "idle,timer", None, |dir, _| {
        let session = dir.join("run/mochi/session/timer");
        std::fs::create_dir_all(&session).unwrap();
        let saved = serde_json::json!({
            "countdown": null,
            "sessions": 0,
            "timers": { "list": [
                { "id": 1, "label": "Tea", "total_ms": 180_000, "ends_ms": 1_000, "left_ms": 180_000 },
                { "id": 2, "label": "", "total_ms": 60_000, "ends_ms": null, "left_ms": 60_000 },
            ] },
        });
        std::fs::write(session.join("timer.json"), saved.to_string()).unwrap();
        let player = dir.join("player");
        let script = format!(
            "#!/bin/sh\necho \"$@\" > {}\n",
            dir.join("played").display()
        );
        std::fs::write(&player, script).unwrap();
        let mut permissions = std::fs::metadata(&player).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&player, permissions).unwrap();
        let config = dir.join("config/mochi");
        std::fs::create_dir_all(&config).unwrap();
        let settings = format!(
            "[module.timer]\nsound_file = \"/tmp/bell.oga\"\nsound_command = [\"{}\"]\n",
            player.display()
        );
        std::fs::write(config.join("config.toml"), settings).unwrap();
    });
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);

    let notice = ui.wait_for_view("timer", "Done");
    assert_eq!(notice.payload["label"], "Tea");
    assert_eq!(notice.payload["length"], "3 min");
    assert_eq!(notice.payload["again"], "3m Tea");
    let played = daemon.dir.join("played");
    daemon.wait_for(|| played.exists(), "the alarm");
    daemon.wait_for(
        || std::fs::read_to_string(&played).is_ok_and(|text| text.trim() == "/tmp/bell.oga"),
        "the alarm's file",
    );
    // The paused one is still there, paused; the media module being off
    // doesn't matter.
    assert_eq!(
        output(ctl.command("timer", "list", &[])),
        "2 Timer 2 1:00 left, paused"
    );
}

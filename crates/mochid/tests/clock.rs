//! The clock module through a real `mochid`: the panel opens on a tab,
//! reminders come and go from the CLI, one that came due while mochid was
//! down comes up after a restart, and the stopwatch carries on from the
//! session directory. When a reminder comes due is the module's unit tests,
//! which don't wait.

mod common;

use common::Daemon;
use mochi_protocol::{DaemonMessage, ErrorCode, Role};

fn output(message: DaemonMessage) -> String {
    match message {
        DaemonMessage::Output { output } => output,
        other => panic!("expected output, got {other:?}"),
    }
}

fn refused(message: &DaemonMessage) -> bool {
    matches!(
        message,
        DaemonMessage::Error {
            code: ErrorCode::ModuleFailed,
            ..
        }
    )
}

#[test]
fn the_panel_opens_on_a_tab() {
    let daemon = Daemon::start("clock-panel", "idle,clock");
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);

    assert_eq!(
        ctl.command("clock", "open", &["calendar"]),
        DaemonMessage::Ok
    );
    let panel = ui.wait_for_view("clock", "Panel");
    assert_eq!(panel.payload["tab"], "calendar");

    // Toggling closes it, and opens it again on the `tab` setting's.
    assert_eq!(ctl.command("clock", "toggle", &[]), DaemonMessage::Ok);
    ui.wait_for_view("idle", "Pill");
    assert_eq!(ctl.command("clock", "toggle", &[]), DaemonMessage::Ok);
    assert_eq!(ui.wait_for_view("clock", "Panel").payload["tab"], "today");
    // A tab that isn't one is refused before the module sees it.
    assert!(matches!(
        ctl.command("clock", "open", &["alarms"]),
        DaemonMessage::Error {
            code: ErrorCode::InvalidArgs,
            ..
        }
    ));
}

#[test]
fn reminders_come_and_go_from_the_cli() {
    let daemon = Daemon::start("clock-reminders", "idle,clock");
    let mut ctl = daemon.client(Role::Ctl);

    assert_eq!(
        output(ctl.command("clock", "reminders", &[])),
        "no reminders"
    );
    let added = output(ctl.command(
        "clock",
        "remind",
        &["tomorrow", "09:30", "Call", "the", "bike", "shop"],
    ));
    assert!(added.starts_with("reminder 0 at "), "{added}");
    assert!(added.ends_with(" 09:30"), "{added}");
    let listed = output(ctl.command("clock", "reminders", &["tomorrow"]));
    assert!(listed.starts_with("0  "), "{listed}");
    assert!(listed.ends_with("09:30  Call the bike shop"), "{listed}");
    assert_eq!(
        output(ctl.command("clock", "reminders", &["today"])),
        "no reminders"
    );

    // Kept in the state directory.
    let file = daemon.dir.join("state/mochi/reminders.json");
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("Call the bike shop"), "{saved}");

    for wrong in [
        &["yesterday", "09:00", "Late"][..],
        &["2020-01-01", "09:00", "Late"],
        &["tomorrow", "25:00", "Never"],
        &["tomorrow", "09:00", " "],
    ] {
        assert!(refused(&ctl.command("clock", "remind", wrong)), "{wrong:?}");
    }

    assert_eq!(ctl.command("clock", "delete", &["0"]), DaemonMessage::Ok);
    assert!(refused(&ctl.command("clock", "delete", &["0"])));
    assert_eq!(
        output(ctl.command("clock", "reminders", &[])),
        "no reminders"
    );
}

#[test]
fn a_reminder_that_came_due_meanwhile_comes_up_after_a_restart() {
    // What the last run kept: a reminder an hour ago, while mochid was
    // down or the computer slept, and one done already.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let daemon = Daemon::start_prepared("clock-restart", "idle,clock", None, |dir, _| {
        let state = dir.join("state/mochi");
        std::fs::create_dir_all(&state).unwrap();
        let saved = serde_json::json!({
            "next": 2,
            "reminders": [
                { "id": 0, "at": now - 7200, "text": "Done already", "done": true },
                { "id": 1, "at": now - 3600, "text": "Water the plants" },
            ],
        });
        std::fs::write(state.join("reminders.json"), saved.to_string()).unwrap();
    });
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);

    let notice = ui.wait_for_view("clock", "Notice");
    assert_eq!(notice.payload["id"], 1);
    assert_eq!(notice.payload["text"], "Water the plants");
    let listed = output(ctl.command("clock", "reminders", &[]));
    assert!(listed.contains("Water the plants (due)"), "{listed}");

    // Snoozing puts it off; Done closes it for good.
    assert_eq!(ctl.command("clock", "snooze", &["1"]), DaemonMessage::Ok);
    ui.wait_for_view("idle", "Pill");
    let listed = output(ctl.command("clock", "reminders", &[]));
    assert!(listed.contains("Water the plants (snoozed to "), "{listed}");
    assert_eq!(ctl.command("clock", "done", &["1"]), DaemonMessage::Ok);
    assert!(refused(&ctl.command("clock", "snooze", &["1"])));
    let listed = output(ctl.command("clock", "reminders", &[]));
    assert!(listed.contains("Water the plants (done)"), "{listed}");
    let saved = std::fs::read_to_string(daemon.dir.join("state/mochi/reminders.json")).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved["reminders"][1]["done"], true);
}

#[test]
fn the_stopwatch_carries_on_after_a_restart() {
    // A stopwatch started a minute ago, with a lap.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let daemon = Daemon::start_prepared("clock-stopwatch", "idle,clock", None, |dir, _| {
        let session = dir.join("run/mochi/session/clock");
        std::fs::create_dir_all(&session).unwrap();
        let saved = serde_json::json!({
            "since_ms": now - 60_000,
            "banked_ms": 0,
            "laps": [20_000],
        });
        std::fs::write(session.join("stopwatch.json"), saved.to_string()).unwrap();
    });
    let mut ctl = daemon.client(Role::Ctl);

    let status = output(ctl.command("clock", "stopwatch", &["status"]));
    let mut lines = status.lines();
    let first = lines.next().unwrap();
    assert!(
        first.starts_with("1:0") && first.ends_with(" running"),
        "{status}"
    );
    assert_eq!(lines.next(), Some("lap 1 0:20.0 0:20.0"));

    assert_eq!(
        ctl.command("clock", "stopwatch", &["lap"]),
        DaemonMessage::Ok
    );
    assert_eq!(
        ctl.command("clock", "stopwatch", &["pause"]),
        DaemonMessage::Ok
    );
    assert!(refused(&ctl.command("clock", "stopwatch", &["lap"])));
    let status = output(ctl.command("clock", "stopwatch", &["status"]));
    assert!(
        status.lines().next().unwrap().ends_with(" paused"),
        "{status}"
    );
    assert_eq!(status.lines().count(), 3, "{status}");

    assert_eq!(
        ctl.command("clock", "stopwatch", &["reset"]),
        DaemonMessage::Ok
    );
    assert_eq!(
        output(ctl.command("clock", "stopwatch", &["status"])),
        "0:00.0 stopped"
    );

    // Reset kept the run, with its two laps, in the state directory.
    let runs = output(ctl.command("clock", "runs", &[]));
    assert!(
        runs.starts_with("1  ") && runs.ends_with(", 2 laps"),
        "{runs}"
    );
    let saved =
        std::fs::read_to_string(daemon.dir.join("state/mochi/stopwatch-runs.json")).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved[0]["laps"].as_array().unwrap().len(), 2);
    assert!(refused(&ctl.command("clock", "copy-run", &["2"])));
    assert_eq!(
        ctl.command("clock", "forget-run", &["1"]),
        DaemonMessage::Ok
    );
    assert_eq!(output(ctl.command("clock", "runs", &[])), "no past runs");
}

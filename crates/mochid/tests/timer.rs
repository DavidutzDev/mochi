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

//! End-to-end tests of `mochid` without a screen: a fake Quickshell keeps the
//! supervisor busy, and the tests connect as the UI and the CLI themselves.

mod common;

use std::thread;
use std::time::Duration;

use common::{Client, Daemon, shown};
use mochi_protocol::{Area, ClientMessage, DaemonMessage, ErrorCode, EventKind, Role, Theme};

fn error_code(message: DaemonMessage) -> ErrorCode {
    match message {
        DaemonMessage::Error { code, .. } => code,
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn ui_gets_the_full_state_after_hello() {
    let daemon = Daemon::start("initial", "idle,demo");
    let mut ui = daemon.client(Role::Ui);

    assert_eq!(
        ui.recv(),
        DaemonMessage::Modules {
            modules: vec!["idle".into(), "demo".into()]
        }
    );
    assert_eq!(
        ui.recv(),
        DaemonMessage::Theme {
            theme: Theme::default()
        }
    );
    let present = ui.next_present();
    assert_eq!(shown(&present), Some(("idle", "Pill")));
    assert_eq!(
        present.unwrap().payload,
        serde_json::json!({ "format": "HH:mm" })
    );
}

#[test]
fn commands_and_events_drive_the_island() {
    let daemon = Daemon::start("island", "idle,demo");
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);
    ui.next_present();

    assert_eq!(
        ctl.command("demo", "show", &["Card", "hello"]),
        DaemonMessage::Ok
    );
    let card = ui.next_present().unwrap();
    assert_eq!((card.view.as_str(), card.expandable), ("Card", true));
    assert_eq!(card.payload, serde_json::json!({ "text": "hello" }));

    // A higher priority interrupts the card.
    assert_eq!(ctl.command("demo", "alert", &["Small"]), DaemonMessage::Ok);
    let alert = ui.next_present().unwrap();
    assert_eq!(alert.view, "Small");

    // Closing the alert brings the card back.
    ui.send(&ClientMessage::Event {
        activity: alert.id,
        kind: EventKind::Dismiss,
    });
    let back = ui.next_present().unwrap();
    assert_eq!(back.id, card.id);

    // A click toggles the expanded view.
    ui.send(&ClientMessage::Event {
        activity: card.id,
        kind: EventKind::Click,
    });
    let expanded = ui.next_present().unwrap();
    assert_eq!(
        (expanded.view.as_str(), expanded.expanded),
        ("CardExpanded", true)
    );

    assert_eq!(ctl.command("demo", "clear", &[]), DaemonMessage::Ok);
    assert_eq!(shown(&ui.next_present()), Some(("idle", "Pill")));
}

#[test]
fn keyed_activities_replace_and_time_out() {
    let daemon = Daemon::start("volume", "idle,demo");
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);
    ui.next_present();

    ctl.command("demo", "volume", &["40"]);
    let first = ui.next_present().unwrap();
    ctl.command("demo", "volume", &["80"]);
    let second = ui.next_present().unwrap();
    assert_eq!(second.view, "Volume");
    assert_ne!(second.id, first.id);
    assert_eq!(second.payload, serde_json::json!({ "level": 80 }));

    // The volume times out after 1.5s, well within the read timeout.
    assert_eq!(shown(&ui.next_present()), Some(("idle", "Pill")));
}

#[test]
fn bubbles_reach_the_ui_and_clicks_reach_the_module() {
    let daemon = Daemon::start("bubbles", "idle,demo");
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);
    assert_eq!(ui.next_bubbles(), []);

    ctl.command("demo", "bubble", &["wifi", "right"]);
    ctl.command("demo", "bubble", &["bt", "left", "status"]);
    // One snapshot per change, unless the daemon got both at once.
    let bubbles = loop {
        let bubbles = ui.next_bubbles();
        if bubbles.len() == 2 {
            break bubbles;
        }
    };
    let placed: Vec<_> = bubbles
        .iter()
        .map(|bubble| {
            (
                bubble.payload["text"].as_str(),
                bubble.area,
                bubble.group.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        placed,
        [
            (Some("bt"), Area::Left, Some("status")),
            (Some("wifi"), Area::Right, None),
        ]
    );

    // Showing a bubble again under the same name moves it, keeping its key.
    ctl.command("demo", "bubble", &["wifi", "center-left"]);
    let moved = ui.next_bubbles();
    assert_eq!(
        (moved[1].area, moved[1].key.as_deref()),
        (Area::CenterLeft, Some("wifi"))
    );

    ui.send(&ClientMessage::BubbleClick {
        bubble: moved[1].id,
    });
    let clicked = ui.next_present().unwrap();
    assert_eq!(
        clicked.payload,
        serde_json::json!({ "text": "wifi clicked" })
    );

    ctl.command("demo", "pop", &["wifi"]);
    assert_eq!(ui.next_bubbles().len(), 1);

    // Only the UI clicks bubbles.
    ctl.send(&ClientMessage::BubbleClick {
        bubble: moved[0].id,
    });
    assert_eq!(error_code(ctl.recv()), ErrorCode::NotAllowed);
}

#[test]
fn bad_requests_get_specific_errors() {
    let daemon = Daemon::start("errors", "idle,demo");
    let mut ctl = daemon.client(Role::Ctl);

    assert_eq!(
        error_code(ctl.command("nope", "show", &[])),
        ErrorCode::UnknownModule
    );
    assert_eq!(
        error_code(ctl.command("demo", "nope", &[])),
        ErrorCode::UnknownAction
    );
    assert_eq!(
        error_code(ctl.command("demo", "show", &["Huge"])),
        ErrorCode::InvalidArgs
    );
    assert_eq!(
        error_code(ctl.command("demo", "volume", &["300"])),
        ErrorCode::ModuleFailed
    );

    ctl.send(&ClientMessage::Event {
        activity: mochi_protocol::ActivityId(1),
        kind: EventKind::Click,
    });
    assert_eq!(error_code(ctl.recv()), ErrorCode::NotAllowed);

    ctl.send_line("{\"type\":\"teleport\"}");
    assert_eq!(error_code(ctl.recv()), ErrorCode::BadMessage);

    let mut early = Client::raw(&daemon.socket);
    early.send(&ClientMessage::Status);
    assert_eq!(error_code(early.recv()), ErrorCode::HelloFirst);
    assert!(early.closed());

    let mut old = Client::raw(&daemon.socket);
    old.send(&ClientMessage::Hello {
        api: 99,
        role: Role::Ctl,
    });
    assert_eq!(error_code(old.recv()), ErrorCode::UnsupportedApi);
    assert!(old.closed());
}

#[test]
fn status_and_actions_describe_the_daemon() {
    let daemon = Daemon::start("status", "idle,demo");
    let mut ctl = daemon.client(Role::Ctl);

    ctl.send(&ClientMessage::Status);
    let DaemonMessage::Status { status } = ctl.recv() else {
        panic!("expected status");
    };
    assert!(!status.ui_connected);
    assert_eq!(status.modules, ["idle", "demo"]);
    // The test daemon can't reach the session's compositor, and says so
    // instead of failing.
    assert_eq!(status.compositor.backend, "unsupported");

    let _ui = daemon.client(Role::Ui);
    ctl.send(&ClientMessage::Status);
    assert!(matches!(ctl.recv(), DaemonMessage::Status { status } if status.ui_connected));

    ctl.send(&ClientMessage::ListActions {
        module: Some("demo".into()),
    });
    let DaemonMessage::Actions { modules } = ctl.recv() else {
        panic!("expected actions");
    };
    let names: Vec<&str> = modules[0]
        .actions
        .iter()
        .map(|action| action.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["show", "alert", "stack", "volume", "bubble", "pop", "clear"]
    );
}

#[test]
fn quickshell_gets_the_socket_and_is_restarted_without_a_hello() {
    let daemon = Daemon::start("handshake", "idle");
    daemon.wait_for(
        || !daemon.quickshell_starts().is_empty(),
        "quickshell to start",
    );

    let starts = daemon.quickshell_starts();
    assert_eq!(starts[0].1, daemon.socket.display().to_string());

    // The fake never says hello, so after the 5s handshake timeout the daemon
    // kills it and starts another.
    thread::sleep(Duration::from_secs(5) + Duration::from_millis(800));
    let starts = daemon.quickshell_starts();
    assert!(starts.len() >= 2, "{starts:?}\n{}", daemon.log());
    assert!(
        !process_exists(starts[0].0),
        "the first quickshell still runs"
    );
}

#[test]
fn sigterm_stops_quickshell_and_removes_the_socket() {
    let mut daemon = Daemon::start("shutdown", "idle");
    daemon.wait_for(
        || !daemon.quickshell_starts().is_empty(),
        "quickshell to start",
    );
    let (pid, _) = daemon.quickshell_starts()[0].clone();

    assert!(daemon.terminate().success(), "{}", daemon.log());
    assert!(!daemon.socket.exists());
    daemon.wait_for(|| !process_exists(pid), "quickshell to exit");
}

#[test]
fn a_second_daemon_refuses_to_start() {
    let daemon = Daemon::start("twice", "idle");
    let second = std::process::Command::new(env!("CARGO_BIN_EXE_mochid"))
        .env("XDG_RUNTIME_DIR", daemon.dir.join("run"))
        .env("XDG_CONFIG_HOME", daemon.dir.join("config"))
        .env("MOCHI_LOG", "error")
        .output()
        .unwrap();
    assert!(!second.status.success());
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("another mochid is running"), "{stderr}");
}

fn process_exists(pid: u32) -> bool {
    // A zombie still has a /proc entry, so check its state too.
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .split_whitespace()
            .nth(2)
            .is_some_and(|state| state == "Z"),
        Err(_) => false,
    }
}

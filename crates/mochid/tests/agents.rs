//! The agents module end to end: a session's states, as `mochi agents
//! hook` sends them, become a bubble and notices on the island.

mod common;

use common::Daemon;
use mochi_protocol::{Bubble, ClientMessage, DaemonMessage, Role};

/// Reads bubbles snapshots until one has the agents' bubble in the state
/// `done` says.
fn agents_bubble(ui: &mut common::Client, done: impl Fn(Option<&Bubble>) -> bool) {
    loop {
        let bubbles = ui.next_bubbles();
        if done(bubbles.iter().find(|bubble| bubble.module == "agents")) {
            return;
        }
    }
}

#[test]
fn a_session_shows_its_states_and_a_click_clears_it_once_done() {
    let daemon = Daemon::start("agents", "idle,agents");
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);
    ui.wait_for_view("idle", "Pill");

    let answer = ctl.command(
        "agents",
        "set",
        &["s1", "working", "Claude Code", "mochi-shell"],
    );
    assert_eq!(answer, DaemonMessage::Ok);
    agents_bubble(&mut ui, |bubble| {
        bubble.is_some_and(|bubble| bubble.payload["sessions"][0]["state"] == "working")
    });

    ctl.command("agents", "set", &["s1", "waiting"]);
    let notice = ui.wait_for_view("agents", "Notice");
    assert_eq!(notice.payload["state"], "waiting");
    // An empty app and title keep the ones it had.
    assert_eq!(notice.payload["app"], "Claude Code");
    assert_eq!(notice.payload["title"], "mochi-shell");

    ctl.command("agents", "set", &["s1", "done"]);
    // The notice and the bubble come in either order.
    let (mut notice, mut bubble) = (None, None);
    while notice.is_none() || bubble.is_none() {
        match ui.recv() {
            DaemonMessage::Present {
                activity: Some(activity),
                ..
            } if activity.view == "Notice" && activity.payload["state"] == "done" => {
                notice = Some(activity);
            }
            DaemonMessage::Bubbles { bubbles, .. } => {
                bubble = bubbles.into_iter().find(|bubble| {
                    bubble.module == "agents" && bubble.payload["sessions"][0]["state"] == "done"
                });
            }
            _ => {}
        }
    }
    assert_eq!(notice.unwrap().payload["title"], "mochi-shell");

    // Every session is done, so a click on the bubble clears them.
    ui.send(&ClientMessage::BubbleClick {
        bubble: bubble.unwrap().id,
    });
    agents_bubble(&mut ui, |bubble| bubble.is_none());
}

#[test]
fn bad_commands_are_refused() {
    let daemon = Daemon::start("agents-refused", "idle,agents");
    let mut ctl = daemon.client(Role::Ctl);
    for args in [&["s1", "sleeping"][..], &["", "working"][..]] {
        assert!(
            matches!(
                ctl.command("agents", "set", args),
                DaemonMessage::Error { .. }
            ),
            "{args:?}"
        );
    }
    assert!(matches!(
        ctl.command("agents", "clear", &["nobody"]),
        DaemonMessage::Error { .. }
    ));
    assert_eq!(ctl.command("agents", "clear-all", &[]), DaemonMessage::Ok);
}

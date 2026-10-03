//! Checks the Wayland backend against the compositor of the current session.
//! It switches a workspace on your screen and back, so it only runs on
//! request:
//!
//!     cargo test -p mochi-compositor --test live -- --ignored --nocapture

#![allow(clippy::print_stderr)]

use std::time::Duration;

use mochi_compositor::{Backend, Compositor, State, WorkspaceId};

const TIMEOUT: Duration = Duration::from_secs(3);

#[tokio::test(flavor = "multi_thread")]
#[ignore = "talks to the session's compositor and switches a workspace; run with --ignored"]
async fn reads_and_switches_workspaces() {
    let compositor = mochi_compositor::connect();
    let state = compositor.state();
    print(&state);

    assert_eq!(
        state.backend,
        Backend::Wayland,
        "ext-workspace-v1 is available"
    );
    assert!(!state.outputs.is_empty(), "the session has outputs");
    for output in &state.outputs {
        assert!(
            state.active_on(&output.name).is_some(),
            "{} has an active workspace",
            output.name
        );
    }

    // Find an output with another workspace to switch to.
    let Some((output, original, other)) = state.outputs.iter().find_map(|output| {
        let active = state.active_on(&output.name)?;
        let other = state
            .workspaces_on(&output.name)
            .find(|workspace| !workspace.active && workspace.can_activate)?;
        Some((output.name.clone(), active.id, other.id))
    }) else {
        eprintln!("no output has a second workspace; skipping the switch");
        return;
    };

    compositor.activate_workspace(other).unwrap();
    wait_until_active(&compositor, &output, other).await;
    eprintln!("switched {output} and saw the compositor confirm it");

    compositor.activate_workspace(original).unwrap();
    wait_until_active(&compositor, &output, original).await;
    eprintln!("switched back");
}

async fn wait_until_active(compositor: &Compositor, output: &str, id: WorkspaceId) {
    let mut updates = compositor.subscribe();
    let wait = updates.wait_for(|state| state.active_on(output).map(|w| w.id) == Some(id));
    tokio::time::timeout(TIMEOUT, wait)
        .await
        .expect("the compositor confirms the switch in time")
        .expect("the backend is still running");
}

fn print(state: &State) {
    eprintln!("backend: {}", state.backend);
    for output in &state.outputs {
        eprintln!("{} ({})", output.name, output.description);
        for workspace in state.workspaces_on(&output.name) {
            let marker = if workspace.active { "*" } else { " " };
            eprintln!("  {marker} {} {:?}", workspace.name, workspace.coordinates);
        }
    }
}

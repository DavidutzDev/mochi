//! Workspace indicator. Shows the workspaces of a monitor when its active
//! workspace changes, when focus moves to it, when one of its workspaces asks
//! for attention, or when workspaces are created or removed. Clicking a dot switches to that
//! workspace.
//!
//! It works on any compositor the compositor adapter supports, and stays
//! idle without one.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.workspaces]
//! timeout_ms = 1200
//! focus = true      # show a monitor when focus moves to it
//! urgent = true     # show workspaces that ask for attention
//! changes = true    # show workspaces being created or removed
//!
//! # Names shown for each monitor when more than one is connected.
//! # Without one, the connector name is shown.
//! [module.workspaces.labels]
//! DP-3 = "Main"
//! HDMI-A-1 = "Side"
//! ```

mod notice;

use std::collections::BTreeMap;
use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::compositor::{Backend, Compositor, StateReceiver};
use mochi_core::{
    ActionSpec, ActivitySpec, ArgSpec, Args, Assets, BoxFuture, Module, ModuleCtx, ModuleError,
    ModuleEvent, Priority, SamePriority,
};
use serde::Deserialize;

use crate::notice::{Notice, Reason, Tracker};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// One slot: a new indicator replaces the shown one in place.
const KEY: &str = "workspaces";

#[derive(Debug, Default)]
pub struct Workspaces;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    timeout_ms: u64,
    focus: bool,
    urgent: bool,
    changes: bool,
    labels: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timeout_ms: 1200,
            focus: true,
            urgent: true,
            changes: true,
            labels: BTreeMap::new(),
        }
    }
}

impl Settings {
    fn shows(&self, notice: &Notice) -> bool {
        match notice.reason {
            Reason::Switch => true,
            Reason::Focus => self.focus,
            Reason::Urgent => self.urgent,
            Reason::Changed => self.changes,
        }
    }
}

impl Module for Workspaces {
    fn id(&self) -> &'static str {
        "workspaces"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("switch", "Switch a monitor to one of its workspaces")
                .arg(ArgSpec::string(
                    "output",
                    "Monitor connector name, like DP-3",
                ))
                .arg(ArgSpec::string("workspace", "Workspace name").rest()),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let timeout = Duration::from_millis(settings.timeout_ms);
            let compositor = ctx.compositor().clone();

            // The current state is the baseline: nothing shows at startup.
            let mut tracker = Tracker::default();
            let mut receiver = compositor.subscribe();
            tracker.apply(&receiver.borrow_and_update());
            let mut updates = Some(receiver);
            if compositor.state().backend == Backend::Unsupported {
                tracing::info!("no workspace information from the compositor; staying idle");
            }

            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            let result = switch(&compositor, &command.args);
                            command.reply(result);
                        }
                        Some(_) => {}
                    },
                    changed = next_change(&mut updates) => {
                        if !changed {
                            // The adapter is gone for good; only commands remain.
                            updates = None;
                            continue;
                        }
                        let Some(receiver) = &mut updates else { continue };
                        let state = receiver.borrow_and_update().clone();
                        if let Some(notice) = tracker.apply(&state)
                            && settings.shows(&notice)
                        {
                            tracing::debug!(?notice, "workspaces");
                            let payload = notice::payload(&notice, &state, &settings.labels);
                            ctx.present(
                                ActivitySpec::new("Workspaces")
                                    .key(KEY)
                                    .priority(Priority::HIGH)
                                    .same_priority(SamePriority::Stack)
                                    // Feedback for a switch you just made:
                                    // clicks go on to your windows.
                                    .passive()
                                    .timeout(timeout)
                                    .payload(payload),
                            );
                        }
                    }
                }
            }
        })
    }
}

/// Waits for the next snapshot. Returns `false` once no more will come, and
/// never returns while there is nothing to listen to.
async fn next_change(updates: &mut Option<StateReceiver>) -> bool {
    match updates {
        Some(updates) => updates.changed().await.is_ok(),
        None => std::future::pending().await,
    }
}

fn switch(compositor: &Compositor, args: &Args) -> Result<(), String> {
    let output = args.str("output").unwrap_or_default();
    let name = args.str("workspace").unwrap_or_default();
    let state = compositor.state();
    if state.backend == Backend::Unsupported {
        return Err("no workspace information from the compositor".into());
    }

    if !state
        .outputs
        .iter()
        .any(|candidate| candidate.name == output)
    {
        let known: Vec<&str> = state
            .outputs
            .iter()
            .map(|output| output.name.as_str())
            .collect();
        return Err(format!(
            "no monitor named {output:?} (monitors: {})",
            known.join(", ")
        ));
    }
    let workspace = state
        .workspaces_on(output)
        .find(|workspace| workspace.name == name)
        .ok_or_else(|| format!("{output} has no workspace named {name:?}"))?;
    compositor
        .activate_workspace(workspace.id)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "workspaces",
            include_str!("../settings.toml"),
        );
    }
}

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
mod tour;

use std::collections::BTreeMap;
use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::compositor::{Backend, Compositor, StateReceiver};
use mochi_core::{
    ActionSpec, ActivitySpec, ArgSpec, Args, Assets, BoxFuture, ContributionSpec, Module,
    ModuleCtx, ModuleError, ModuleEvent, Priority, SamePriority,
};
use serde::Deserialize;

use crate::notice::{Notice, Reason, Tracker};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// One slot: a new indicator replaces the shown one in place.
const KEY: &str = "workspaces";
/// The dots `show` brings up, apart from the indicator: a switch made from
/// them updates them in place instead of replacing them with the
/// indicator, which lets clicks through.
const SHOW_KEY: &str = "workspaces-show";

#[derive(Debug, Default)]
pub struct Workspaces;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
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

    fn contributions(&self) -> Vec<ContributionSpec> {
        tour::steps()
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<serde_json::Value> {
        Some(mochi_core::options::schema_of::<Settings>())
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
            ActionSpec::new(
                "show",
                "Show a monitor's workspaces on the island, to click or scroll through",
            )
            .arg(
                ArgSpec::string(
                    "output",
                    "Monitor connector name; the one under the pointer when left out",
                )
                .optional(),
            ),
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
            // The dots `show` brought up, and their monitor.
            let mut shown: Option<(mochi_core::ActivityId, String)> = None;
            if compositor.state().backend == Backend::Unsupported {
                tracing::info!("no workspace information from the compositor; staying idle");
            }

            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            let result = match command.action.as_str() {
                                "show" => show(&ctx, &compositor, &command.args, &settings, timeout)
                                    .map(|activity| shown = Some(activity)),
                                _ => switch(&compositor, &command.args),
                            };
                            command.reply(result);
                        }
                        Some(ModuleEvent::Ended { activity, .. })
                            if shown.as_ref().is_some_and(|(id, _)| *id == activity) =>
                        {
                            shown = None;
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
                        let notice = tracker.apply(&state);
                        // While the dots show, they follow the switches.
                        if let Some((activity, output)) = &shown {
                            ctx.update(*activity, overview(output, &state, &settings.labels));
                            continue;
                        }
                        if let Some(notice) = notice
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
                                    .fleeting()
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

/// What `show` shows: a monitor's workspaces, as the indicator has them.
fn overview(
    output: &str,
    state: &mochi_core::compositor::State,
    labels: &BTreeMap<String, String>,
) -> serde_json::Value {
    let notice = Notice {
        output: output.to_owned(),
        reason: Reason::Focus,
        urgent: None,
    };
    notice::payload(&notice, state, labels)
}

/// Shows a monitor's workspaces on the island: the given one, or the one
/// under the pointer, else the focused one. They stay while the pointer is
/// on them, take clicks, and go a moment after it leaves.
fn show(
    ctx: &ModuleCtx,
    compositor: &Compositor,
    args: &Args,
    settings: &Settings,
    timeout: Duration,
) -> Result<(mochi_core::ActivityId, String), String> {
    let state = compositor.state();
    if state.backend == Backend::Unsupported {
        return Err("no workspace information from the compositor".into());
    }
    let output = args
        .str("output")
        .map(str::to_owned)
        .or_else(|| compositor.pointer_output())
        .or_else(|| state.focused_output.clone())
        .or_else(|| state.outputs.first().map(|output| output.name.clone()))
        .ok_or("no monitors")?;
    if !state
        .outputs
        .iter()
        .any(|candidate| candidate.name == output)
    {
        return Err(format!("no monitor named {output:?}"));
    }
    let activity = ctx.present(
        ActivitySpec::new("Workspaces")
            .key(SHOW_KEY)
            .priority(Priority::HIGH)
            .fleeting()
            .output(output.clone())
            .timeout(timeout)
            .payload(overview(&output, &state, &settings.labels)),
    );
    Ok((activity, output))
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

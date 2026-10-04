//! App launcher. `mochi ipc launcher toggle`, bound to a key, grows the
//! island into a search box over your apps: most used first, fuzzy search as
//! you type, and app actions like "New Private Window" when you search for
//! them. Enter or a click starts the app; Escape or a click elsewhere closes
//! it.
//!
//! Apps start through `uwsm app` in a uwsm session, otherwise through
//! `systemd-run --user --scope`, so they never belong to mochid.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.launcher]
//! # Runs terminal apps when uwsm doesn't. Defaults to xdg-terminal-exec when
//! # installed, otherwise $TERMINAL -e.
//! terminal = ["foot", "-e"]
//! max_results = 50
//! ```

mod entries;
mod history;
mod launch;
mod search;

use std::time::SystemTime;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, Module,
    ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::entries::{App, Locale};
use crate::history::History;
use crate::launch::Method;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Launcher;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    terminal: Vec<String>,
    max_results: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            terminal: Vec::new(),
            max_results: 50,
        }
    }
}

impl Module for Launcher {
    fn id(&self) -> &'static str {
        "launcher"
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
            ActionSpec::new("toggle", "Open the launcher, or close it when open"),
            ActionSpec::new("open", "Open the launcher"),
            ActionSpec::new("close", "Close the launcher"),
            ActionSpec::new(
                "search",
                "Search for apps; the launcher sends this as you type",
            )
            .arg(
                ArgSpec::string("query", "What to look for")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new("launch", "Start an app by desktop id, like firefox.desktop")
                .arg(ArgSpec::string("id", "Desktop id, or id:action")),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let method = Method::detect().await;
            tracing::info!(?method, "starting apps");
            let mut state = State {
                terminal: if settings.terminal.is_empty() {
                    default_terminal()
                } else {
                    settings.terminal.clone()
                },
                max_results: settings.max_results,
                method,
                history: History::default_path()
                    .map(History::load)
                    .unwrap_or_default(),
                apps: Vec::new(),
                query: String::new(),
                shown: None,
            };

            while let Some(event) = ctx.next_event().await {
                match event {
                    ModuleEvent::Command(command) => state.command(&ctx, command).await,
                    ModuleEvent::Ended { activity, .. } if state.shown == Some(activity) => {
                        state.shown = None;
                    }
                    _ => {}
                }
            }
            Ok(())
        })
    }
}

#[derive(Debug)]
struct State {
    terminal: Vec<String>,
    max_results: usize,
    method: Method,
    history: History,
    /// Read again every time the launcher opens, so new installs show up.
    apps: Vec<App>,
    query: String,
    shown: Option<ActivityId>,
}

impl State {
    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let result = match command.action.as_str() {
            "toggle" if self.shown.is_some() => {
                self.close(ctx);
                Ok(())
            }
            "toggle" | "open" => {
                self.open(ctx).await;
                Ok(())
            }
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "search" => {
                self.query = command.args.str("query").unwrap_or_default().to_owned();
                if let Some(id) = self.shown {
                    ctx.update(id, self.payload(ctx));
                }
                Ok(())
            }
            "launch" => {
                let id = command.args.str("id").unwrap_or_default().to_owned();
                self.launch(ctx, &id).await
            }
            other => Err(format!("launcher has no action {other}")),
        };
        command.reply(result);
    }

    async fn open(&mut self, ctx: &ModuleCtx) {
        // The hub and the clipboard take the keyboard too; only one can be
        // open. Not awaited: they close the launcher the same way.
        for module in ["hub", "clipboard"] {
            let close = ctx.call(module, "close", &[]);
            tokio::spawn(async move {
                match close.await {
                    Ok(()) | Err(CallError::NotEnabled(_)) => {}
                    Err(error) => tracing::warn!(%error, module, "could not close it"),
                }
            });
        }

        self.apps = read_apps().await;
        self.query.clear();
        let spec = ActivitySpec::new("Launcher")
            .key("launcher")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(self.payload(ctx));
        self.shown = Some(ctx.present(spec));
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown.take() {
            ctx.withdraw(id);
        }
    }

    async fn launch(&mut self, ctx: &ModuleCtx, id: &str) -> Result<(), String> {
        if self.apps.is_empty() {
            self.apps = read_apps().await;
        }
        let (app_id, action_id) = match id.split_once(':') {
            Some((app, action)) => (app, Some(action)),
            None => (id, None),
        };
        let app = self
            .apps
            .iter()
            .find(|app| app.id == app_id)
            .ok_or_else(|| format!("no app {app_id}"))?;
        let action = match action_id {
            Some(action_id) => Some(
                app.actions
                    .iter()
                    .find(|action| action.id == action_id)
                    .ok_or_else(|| format!("{} has no action {action_id}", app.name))?,
            ),
            None => None,
        };
        launch::launch(self.method, app, action, &self.terminal)?;
        tracing::info!(id, "launched");
        self.history.record(app_id, now());
        self.close(ctx);
        Ok(())
    }

    fn payload(&self, ctx: &ModuleCtx) -> Value {
        let hits = search::rank(
            &self.apps,
            &self.history,
            &self.query,
            now(),
            self.max_results,
        );
        json!({
            // Only the island on this monitor takes the keyboard.
            "output": ctx.compositor().state().focused_output,
            "query": self.query,
            "results": hits
                .iter()
                .map(|hit| json!({
                    "id": hit.id,
                    "name": hit.name,
                    "description": hit.description,
                    "icon": hit.icon,
                    "action": hit.action,
                }))
                .collect::<Vec<_>>(),
        })
    }
}

async fn read_apps() -> Vec<App> {
    let desktops: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|desktop| !desktop.is_empty())
        .map(str::to_owned)
        .collect();
    let directories = entries::directories();
    tokio::task::spawn_blocking(move || entries::scan(&directories, &Locale::from_env(), &desktops))
        .await
        .unwrap_or_default()
}

/// `xdg-terminal-exec` when installed, otherwise `$TERMINAL -e`.
fn default_terminal() -> Vec<String> {
    if entries::installed("xdg-terminal-exec") {
        return vec!["xdg-terminal-exec".into()];
    }
    match std::env::var("TERMINAL") {
        Ok(terminal) if !terminal.is_empty() => vec![terminal, "-e".into()],
        _ => Vec::new(),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "launcher",
            include_str!("../settings.toml"),
        );
    }
}

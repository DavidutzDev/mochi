//! Updates: looks for a new Mochi release on GitHub at start and every
//! `interval_hours`, says so once per version on the island, and has a page
//! in the settings with the changelog of each newer release. How it updates
//! depends on how Mochi was installed: the install script runs again and
//! Mochi restarts, a package manager or Nix gets its command to copy or
//! run in a terminal.
//!
//! Settings in `config.toml`:
//!
//! ```toml
//! [module.updater]
//! check = true
//! interval_hours = 12
//! command = []    # your own update command, like ["nh", "os", "switch", "--update"]
//! terminal = []   # like ["kitty"]; xdg-terminal-exec or $TERMINAL by default
//! ```

mod install;
mod releases;
mod tour;

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, ContributionSpec,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use crate::install::Method;
use crate::releases::Release;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long after start the first check waits, so it doesn't compete with
/// everything starting.
const FIRST_CHECK: Duration = Duration::from_secs(8);
/// How long the notice stays.
const NOTICE: Duration = Duration::from_secs(15);

#[derive(Debug, Default)]
pub struct Updater;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// Look for a new release on GitHub at start and every `interval_hours`.
    /// Nothing is sent but the request for the list of releases.
    check: bool,
    /// Hours between two checks while Mochi runs.
    #[schemars(range(min = 1, max = 168))]
    interval_hours: u64,
    /// The command "Run in a terminal" runs instead of the one for how
    /// Mochi was installed, like ["nh", "os", "switch", "--update"].
    command: Vec<String>,
    /// The terminal that runs it, like ["kitty"] or ["foot"]. Empty uses
    /// xdg-terminal-exec, or $TERMINAL.
    #[schemars(extend("x-suggest" = [["kitty"], ["foot"], ["alacritty"], ["ghostty"], ["wezterm", "start"], ["konsole", "-e"], ["gnome-terminal", "--"], ["xterm", "-e"]]))]
    terminal: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            check: true,
            interval_hours: 12,
            command: Vec::new(),
            terminal: Vec::new(),
        }
    }
}

/// `$XDG_STATE_HOME/mochi/updater.json`: the newest version the island
/// already told about, so it tells once.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    told: String,
}

impl Saved {
    fn path() -> Option<PathBuf> {
        Some(mochi_core::config::state_dir()?.join("updater.json"))
    }

    fn load() -> Self {
        Self::path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::write(&path, serde_json::to_vec_pretty(self).unwrap_or_default())
            });
        if let Err(error) = written {
            tracing::warn!(%error, "can't save the updater's state");
        }
    }
}

/// What a task the module started came back with.
enum Done {
    Checked(Result<Vec<Release>, String>),
    Updated(Result<(), String>, String),
}

/// An update by the install script, while it runs and after.
#[derive(Debug, Default)]
struct Job {
    running: bool,
    /// What the installer printed.
    log: String,
    error: Option<String>,
}

impl Module for Updater {
    fn id(&self) -> &'static str {
        "updater"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("check", "Look for a new release now"),
            ActionSpec::new("view", "Open the Updates page in the settings"),
            ActionSpec::new("skip", "Close the notice about a new release"),
            ActionSpec::new(
                "update",
                "Update with the install script and restart, when it installed Mochi",
            ),
            ActionSpec::new("run", "Open a terminal running the update command"),
            ActionSpec::new("copy", "Copy the update command"),
            ActionSpec::new("open-release", "Open a release's page on GitHub")
                .arg(ArgSpec::string("version", "A newer release, like 0.0.9")),
        ]
    }

    fn needs(&self, table: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        let settings: Settings = mochi_core::settings(table).unwrap_or_default();
        let mut needs = Vec::new();
        if settings.check {
            needs.push(mochi_core::Need::new("curl", "Looking for a new release"));
        }
        needs.push(mochi_core::Need::new(
            "xdg-open",
            "Opening a release's page on GitHub",
        ));
        needs
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![ContributionSpec::new(
            "settings", "section", "updates", "Page", "Updates",
        )];
        offers.extend(tour::steps());
        offers
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let (done, mut finished) = tokio::sync::mpsc::unbounded_channel();
            let mut state = State {
                method: Method::detect(),
                saved: Saved::load(),
                interval: Duration::from_secs(settings.interval_hours.max(1) * 3600),
                settings,
                done,
                checking: false,
                checked_at: None,
                error: None,
                newer: Vec::new(),
                notice: None,
                job: Job::default(),
            };
            state.publish(&ctx);
            let mut next = state
                .settings
                .check
                .then(|| tokio::time::Instant::now() + FIRST_CHECK);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::Ended { activity, .. }) if state.notice == Some(activity) => {
                            state.notice = None;
                        }
                        Some(_) => {}
                    },
                    Some(done) = finished.recv() => state.finished(&ctx, done),
                    () = sleep_until(next) => {
                        state.check();
                        next = Some(tokio::time::Instant::now() + state.interval);
                    }
                }
                state.publish(&ctx);
            }
        })
    }
}

async fn sleep_until(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

struct State {
    settings: Settings,
    method: Method,
    saved: Saved,
    interval: Duration,
    done: UnboundedSender<Done>,
    checking: bool,
    /// When the last check ended, in seconds since the epoch.
    checked_at: Option<u64>,
    error: Option<String>,
    /// The releases newer than this one, newest first.
    newer: Vec<Release>,
    notice: Option<ActivityId>,
    job: Job,
}

impl State {
    fn check(&mut self) {
        if self.checking {
            return;
        }
        self.checking = true;
        let done = self.done.clone();
        tokio::spawn(async move {
            let result = match releases::fetch().await {
                Ok(json) => releases::newer(&json, mochi_core::version::VERSION),
                Err(error) => Err(error),
            };
            let _ = done.send(Done::Checked(result));
        });
    }

    fn finished(&mut self, ctx: &ModuleCtx, done: Done) {
        match done {
            Done::Checked(result) => {
                self.checking = false;
                self.checked_at = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|now| now.as_secs());
                match result {
                    Ok(newer) => {
                        self.error = None;
                        self.newer = newer;
                        self.tell(ctx);
                    }
                    Err(error) => {
                        tracing::info!(%error, "no new release found");
                        self.error = Some(error);
                    }
                }
            }
            Done::Updated(result, log) => {
                self.job.running = false;
                self.job.log = log;
                match result {
                    Ok(()) => {
                        let exe = match &self.method {
                            Method::Script { prefix } => prefix.join("bin/mochid"),
                            _ => install::executable().unwrap_or_default(),
                        };
                        tracing::info!("updated, restarting");
                        if let Err(error) = install::restart(&exe) {
                            self.job.error = Some(format!(
                                "updated, but can't restart: {error}. Restart Mochi to use it."
                            ));
                        }
                    }
                    Err(error) => self.job.error = Some(error),
                }
            }
        }
    }

    /// A notice on the island, once per version.
    fn tell(&mut self, ctx: &ModuleCtx) {
        let Some(latest) = self.newer.first() else {
            return;
        };
        if self.saved.told == latest.version || self.notice.is_some() {
            return;
        }
        let spec = ActivitySpec::new("Notice")
            .key("notice")
            .priority(Priority::NORMAL)
            .timeout(NOTICE)
            .payload(json!({
                "version": latest.version,
                "current": mochi_core::version::VERSION,
            }));
        self.notice = Some(ctx.present(spec));
        self.saved.told = latest.version.clone();
        self.saved.save();
    }

    fn close_notice(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.notice.take() {
            ctx.withdraw(id);
        }
    }

    /// The update command: the one from the settings, or the method's.
    fn command_line(&self) -> Vec<String> {
        if self.settings.command.is_empty() {
            self.method.command().0
        } else {
            self.settings.command.clone()
        }
    }

    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let result = match command.action.as_str() {
            "check" => {
                self.check();
                Ok(())
            }
            "view" => {
                self.close_notice(ctx);
                let open = ctx.call("settings", "open", &["updater"]);
                tokio::spawn(async move {
                    match open.await {
                        Ok(()) => {}
                        Err(CallError::NotEnabled(_)) => {
                            tracing::info!("the settings module is off: no Updates page to open");
                        }
                        Err(error) => tracing::warn!(%error, "can't open the Updates page"),
                    }
                });
                Ok(())
            }
            "skip" => {
                self.close_notice(ctx);
                Ok(())
            }
            "update" => self.update(),
            "run" => match install::terminal(&self.settings.terminal) {
                Some(terminal) => install::run_in_terminal(&terminal, &self.command_line()),
                None => {
                    Err("no terminal found: set `terminal`, or install xdg-terminal-exec".into())
                }
            },
            "copy" => {
                copy(ctx, install::line(&self.command_line()));
                Ok(())
            }
            "open-release" => {
                let version = command.args.str("version").unwrap_or_default();
                match self
                    .newer
                    .iter()
                    .find(|release| release.version == version && !release.url.is_empty())
                {
                    Some(release) => mochi_core::process::spawn_detached(
                        &mochi_core::process::in_app_scope(&[
                            "xdg-open".into(),
                            release.url.clone(),
                        ]),
                        None,
                    ),
                    None => Err(format!("no newer release {version:?}")),
                }
            }
            other => Err(format!("updater has no action {other}")),
        };
        command.reply(result);
    }

    /// Runs the install script again, for a Mochi it installed.
    fn update(&mut self) -> Result<(), String> {
        let Method::Script { .. } = self.method else {
            return Err(
                "this Mochi didn't come from the install script: run its update command".into(),
            );
        };
        if self.job.running {
            return Err("an update already runs".into());
        }
        self.job = Job {
            running: true,
            ..Job::default()
        };
        // Under systemd, in a scope of its own, so the installer restarting
        // the service doesn't stop it on the way.
        let command = self.method.command().0;
        let argv = if std::env::var_os("INVOCATION_ID").is_some() {
            mochi_core::process::in_app_scope(&command)
        } else {
            command
        };
        let done = self.done.clone();
        tokio::spawn(async move {
            let run = tokio::process::Command::new(&argv[0])
                .args(&argv[1..])
                .stdin(std::process::Stdio::null())
                .kill_on_drop(true)
                .output();
            let (result, log) = match tokio::time::timeout(Duration::from_secs(15 * 60), run).await
            {
                Err(_) => (
                    Err("the install script took over 15 minutes".to_owned()),
                    String::new(),
                ),
                Ok(Err(error)) => (
                    Err(format!("can't run the install script: {error}")),
                    String::new(),
                ),
                Ok(Ok(output)) => {
                    let mut log = String::from_utf8_lossy(&output.stdout).into_owned();
                    log.push_str(&String::from_utf8_lossy(&output.stderr));
                    let log = log.trim().to_owned();
                    let result = if output.status.success() {
                        Ok(())
                    } else {
                        Err(log
                            .lines()
                            .rev()
                            .find(|line| !line.trim().is_empty())
                            .unwrap_or("the install script failed")
                            .trim_start_matches("mochi installer: ")
                            .to_owned())
                    };
                    (result, log)
                }
            };
            let _ = done.send(Done::Updated(result, log));
        });
        Ok(())
    }

    fn publish(&self, ctx: &ModuleCtx) {
        let (command, about) = self.method.command();
        let command = if self.settings.command.is_empty() {
            command
        } else {
            self.settings.command.clone()
        };
        ctx.publish_state(json!({
            "current": mochi_core::version::VERSION,
            "latest": self.newer.first().map(|release| release.version.clone()),
            "newer": self.newer,
            "check": self.settings.check,
            "checking": self.checking,
            "checked_at": self.checked_at,
            "error": self.error,
            "method": self.method,
            "about": about,
            "command": install::line(&command),
            "custom": !self.settings.command.is_empty(),
            "terminal": install::terminal(&self.settings.terminal).is_some(),
            "job": {
                "running": self.job.running,
                "log": self.job.log,
                "error": self.job.error,
            },
        }));
    }
}

/// Copies `text`, with the clipboard module or wl-copy.
fn copy(ctx: &ModuleCtx, text: String) {
    let call = ctx.call("clipboard", "copy-text", &[&text]);
    tokio::spawn(async move {
        match call.await {
            Ok(()) => {}
            Err(CallError::NotEnabled(_)) => {
                if let Err(error) = mochi_core::process::spawn_detached(
                    &["wl-copy".into(), "--".into(), text],
                    None,
                ) {
                    tracing::warn!(%error, "can't copy: enable the clipboard module or install wl-copy");
                }
            }
            Err(error) => tracing::warn!(%error, "the clipboard module couldn't copy it"),
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_example_matches_the_settings() {
        mochi_core::examples::check_module::<super::Settings>(
            "updater",
            include_str!("../settings.toml"),
        );
    }
}

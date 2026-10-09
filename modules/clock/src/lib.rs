//! The clock panel: `mochi ipc clock toggle` opens a panel on the island
//! with five tabs. Today has the time, the weather from the weather module
//! and the next reminder; Calendar a month with the reminders of each day;
//! Timer drives the timer module; Stopwatch counts up with laps; World has
//! the time in other zones.
//!
//! Reminders are kept in `$XDG_STATE_HOME/mochi/reminders.json`, see
//! [`reminders`]. When one comes due, the island says so, with Done and
//! Snooze; one that came due while mochid was down or the computer slept
//! comes up as soon as either is back. The stopwatch is kept in the session
//! directory, so it runs on through a restart, like the timer; resetting it
//! keeps the run in `$XDG_STATE_HOME/mochi/stopwatch-runs.json`.
//!
//! The views count the stopwatch up themselves from when it started, so
//! the daemon only speaks when something changes.
//!
//! The control center gets a card, with the time and the next reminder or
//! the stopwatch, and a page with the panel's tabs but the timer's. The
//! clock's page in the settings credits mochi-clock, by Xonex5, which some
//! of the clock's features come from.
//!
//! Settings in `config.toml`, all optional and applied without a restart:
//!
//! ```toml
//! [module.clock]
//! tab = "today"            # the tab it opens on
//! zones = ["Europe/London", "America/New_York", "Asia/Tokyo"]
//! hours = "24"             # or "12"
//! first_day = "monday"     # or "sunday"
//! seconds = false          # the seconds on Today
//! day_progress = "line"    # "ring" or "none"
//! shape = "cookie"         # what Today's clock sits in, or "none"
//! precision = "tenths"     # or "hundredths", for the stopwatch
//! history = 10             # the stopwatch's past runs it keeps
//! ```

mod local;
mod reminders;
mod stopwatch;
mod tour;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use include_dir::{Dir, include_dir};
use mochi_core::zones::{self, Offsets};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, ContributionSpec,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority, SettingsOp,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::reminders::{Reminder, Reminders};
use crate::stopwatch::{MOST_RUNS, Precision, Runs, Stopwatch};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The panel's tabs, in order.
const TABS: [&str; 5] = ["today", "calendar", "timer", "stopwatch", "world"];
/// The file in the session directory that keeps the stopwatch across
/// restarts.
const STOPWATCH: &str = "stopwatch.json";
/// How long a reminder's notice stays on screen. A reminder nobody marked
/// done stays due on the Today tab and in the calendar.
const NOTICE: Duration = Duration::from_secs(60);
/// How long the daemon waits at most between two looks at the clock while a
/// reminder is ahead, so one that came due during a suspend comes up soon
/// after waking.
const LOOK: Duration = Duration::from_secs(10);
/// How long Snooze puts a reminder off by default, in minutes.
const SNOOZE: i64 = 10;
/// The most zones the World tab shows.
const MOST_ZONES: usize = 8;
/// Every setting is read by the views or the next look at the clock.
const LIVE: [&str; 9] = [
    "tab",
    "zones",
    "hours",
    "first_day",
    "seconds",
    "day_progress",
    "shape",
    "precision",
    "history",
];
/// mochi-clock, by Xonex5, which inspired the clock's seconds, day
/// progress, stopwatch runs and city list. The credit on the settings page
/// opens it, and nothing else.
const CREDIT: &str = "https://github.com/Xonex5/mochi-clock";

#[derive(Debug, Default)]
pub struct Clock;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
enum Tab {
    /// The time, the weather and the next reminder.
    #[default]
    Today,
    /// A month with its reminders.
    Calendar,
    /// The focus timer.
    Timer,
    /// The stopwatch and its laps.
    Stopwatch,
    /// The time in other zones.
    World,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
enum Hours {
    /// 0 to 23.
    #[default]
    #[serde(rename = "24")]
    TwentyFour,
    /// 1 to 12, with AM and PM.
    #[serde(rename = "12")]
    Twelve,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
enum FirstDay {
    #[default]
    Monday,
    Sunday,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
enum DayProgress {
    /// A thin line under the date.
    #[default]
    Line,
    /// A ring around the clock.
    Ring,
    /// Neither.
    None,
}

/// The shapes of ExpressiveShape a clock can sit in.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
enum Shape {
    /// No shape: the time in the text's color.
    None,
    /// A circle.
    Circle,
    /// A pentagon with round corners.
    Pentagon,
    /// A cookie with soft lobes.
    #[default]
    Cookie,
    /// Four round lobes.
    Clover,
    /// A burst with soft points.
    Burst,
    /// A hexagon with round corners.
    Hexagon,
    /// An octagon with round corners.
    Octagon,
    /// A square with soft sides.
    Squircle,
    /// A wide pill.
    Pill,
}

#[derive(Debug, Clone, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// The tab the panel opens on.
    tab: Tab,
    /// The time zones on the World tab, like "Europe/Paris", up to 8.
    zones: Vec<String>,
    /// A 24-hour or a 12-hour clock.
    hours: Hours,
    /// The day weeks start on in the calendar.
    first_day: FirstDay,
    /// Show the seconds on the Today tab.
    seconds: bool,
    /// How far through the day it is, on the Today tab.
    day_progress: DayProgress,
    /// The shape the Today tab's clock sits in.
    shape: Shape,
    /// How finely the stopwatch shows the time.
    precision: Precision,
    /// How many of the stopwatch's past runs to keep, up to 50; 0 keeps
    /// none. Reset keeps a run.
    history: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tab: Tab::Today,
            zones: zones::DEFAULT.map(str::to_owned).to_vec(),
            hours: Hours::TwentyFour,
            first_day: FirstDay::Monday,
            seconds: false,
            day_progress: DayProgress::Line,
            shape: Shape::Cookie,
            precision: Precision::Tenths,
            history: 10,
        }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let mut settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        settings.zones = zones::list(&json!(settings.zones));
        if settings.zones.len() > MOST_ZONES {
            return Err(format!(
                "zones lists {}; the World tab shows up to {MOST_ZONES}",
                settings.zones.len()
            ));
        }
        if settings.history > MOST_RUNS {
            return Err(format!(
                "history is {}; the stopwatch keeps up to {MOST_RUNS} runs",
                settings.history
            ));
        }
        Ok(settings)
    }
}

impl Module for Clock {
    fn id(&self) -> &'static str {
        "clock"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        Settings::load(table).map(drop)
    }

    fn live_settings(&self) -> &'static [&'static str] {
        &LIVE
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let id = |help: &str| ArgSpec::int("id", help);
        vec![
            ActionSpec::new("toggle", "Open the clock panel, or close it when open"),
            ActionSpec::new("open", "Open the clock panel, or switch its tab").arg(
                ArgSpec::choice("tab", "The tab, instead of the `tab` setting", TABS).optional(),
            ),
            ActionSpec::new("close", "Close the clock panel"),
            ActionSpec::new("remind", "Add a reminder")
                .arg(ArgSpec::string(
                    "date",
                    "Like 2026-10-21, today or tomorrow",
                ))
                .arg(ArgSpec::string("time", "Like 20:00"))
                .arg(ArgSpec::string("text", "What to remind you of").rest()),
            ActionSpec::new("reminders", "Print the reminders, or a day's")
                .arg(ArgSpec::string("date", "Like 2026-10-21, today or tomorrow").optional()),
            ActionSpec::new("delete", "Delete a reminder").arg(id("Its number, from reminders")),
            ActionSpec::new("done", "Mark a reminder done, so it doesn't come up again")
                .arg(id("Its number, from reminders")),
            ActionSpec::new("snooze", "Bring a due reminder up again later")
                .arg(id("Its number, from reminders"))
                .arg(ArgSpec::int("minutes", "How much later, 10 by default").optional()),
            ActionSpec::new("stopwatch", "Start, pause, lap or reset the stopwatch").arg(
                ArgSpec::choice(
                    "what",
                    "What to do; status prints its time and laps",
                    ["start", "pause", "toggle", "lap", "reset", "status"],
                ),
            ),
            ActionSpec::new(
                "runs",
                "Print the stopwatch's past runs, with their numbers",
            ),
            ActionSpec::new(
                "copy-run",
                "Copy a stopwatch run with its laps: the one going, or a past one",
            )
            .arg(ArgSpec::int("run", "Its number, from runs").optional()),
            ActionSpec::new(
                "forget-run",
                "Forget one of the stopwatch's past runs, or all of them",
            )
            .arg(ArgSpec::int("run", "Its number, from runs").optional()),
            ActionSpec::new("add-zone", "Add a time zone to the World tab")
                .arg(ArgSpec::string("zone", "Like Europe/Paris")),
            ActionSpec::new("remove-zone", "Take a time zone off the World tab")
                .arg(ArgSpec::string("zone", "Like Europe/Paris")),
            ActionSpec::new(
                "credit",
                "Open mochi-clock, by Xonex5, which inspired the clock, in the browser",
            ),
        ]
    }

    fn needs(&self, _settings: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        vec![mochi_core::Need::new(
            "xdg-open",
            "The link to mochi-clock on the clock's settings page",
        )]
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("settings", "section", "credit", "Credit", "Credit"),
            // After the control center's own Today card.
            ContributionSpec::new("control-center", "card", "clock", "Card", "Clock")
                .icon("clock")
                .order(4)
                .options(json!({ "span": 1, "rows": 1, "page": "clock" })),
            ContributionSpec::new("control-center", "page", "clock", "Page", "Clock")
                .icon("clock")
                .order(14),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused what doesn't load.
            let table: mochi_core::toml::Table = ctx.settings()?;
            let settings = Settings::load(&table)?;
            let path = Reminders::path();
            let runs_path = Runs::path();
            let session = ctx.session_dir().to_owned();
            let mut state = State {
                reminders: path.as_deref().map(Reminders::load).unwrap_or_default(),
                path,
                runs: runs_path.as_deref().map(Runs::load).unwrap_or_default(),
                runs_path,
                stopwatch: load_stopwatch(&session),
                session,
                zones: Offsets::read(settings.zones.clone()).await,
                settings,
                panel: None,
                notices: BTreeMap::new(),
                told: BTreeSet::new(),
            };
            state.publish(&ctx);
            let mut refresh = tokio::time::interval(zones::REFRESH);
            refresh.tick().await;
            loop {
                // Reminders that came due while mochid was down come up at
                // once.
                state.tell_due(&ctx);
                let wait = state.wait(seconds());
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::Ended { activity, .. }) => state.ended(activity),
                        Some(ModuleEvent::Reconfigured(table)) => match Settings::load(&table) {
                            Ok(settings) => {
                                let fewer = settings.history < state.settings.history;
                                state.settings = settings;
                                if fewer {
                                    state.runs.0.truncate(state.settings.history as usize);
                                    state.save_runs();
                                }
                                if state.zones.missing(&state.settings.zones) {
                                    state.zones = Offsets::read(state.settings.zones.clone()).await;
                                }
                                state.publish(&ctx);
                            }
                            Err(error) => tracing::warn!(%error, "the clock's new settings"),
                        },
                        Some(_) => {}
                    },
                    () = sleep(wait) => {}
                    _ = refresh.tick() => {
                        state.zones = Offsets::read(state.settings.zones.clone()).await;
                        state.publish(&ctx);
                    }
                }
            }
        })
    }
}

async fn sleep(wait: Option<Duration>) {
    match wait {
        Some(wait) => tokio::time::sleep(wait).await,
        None => std::future::pending().await,
    }
}

/// The wall clock, in seconds since the epoch.
fn seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// The wall clock, in milliseconds since the epoch.
fn millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

fn load_stopwatch(dir: &Path) -> Stopwatch {
    let Ok(text) = std::fs::read_to_string(dir.join(STOPWATCH)) else {
        return Stopwatch::default();
    };
    serde_json::from_str(&text)
        .inspect_err(|error| tracing::warn!(%error, "ignoring the saved stopwatch"))
        .unwrap_or_default()
}

fn save_stopwatch(dir: &Path, stopwatch: &Stopwatch) {
    let written = std::fs::create_dir_all(dir).and_then(|()| {
        let temporary = dir.join(format!("{STOPWATCH}.new"));
        std::fs::write(
            &temporary,
            serde_json::to_vec(stopwatch).unwrap_or_default(),
        )?;
        std::fs::rename(temporary, dir.join(STOPWATCH))
    });
    if let Err(error) = written {
        tracing::warn!(%error, "can't save the stopwatch");
    }
}

/// Copies `text` through the clipboard module, or with wl-copy when it's
/// off.
fn copy(ctx: &ModuleCtx, text: String) {
    let call = ctx.call("clipboard", "copy-text", &[&text]);
    tokio::spawn(async move {
        match call.await {
            Ok(()) => {}
            Err(CallError::NotEnabled(_)) => {
                let copied = mochi_core::process::spawn_detached(
                    &["wl-copy".into(), "--".into(), text],
                    None,
                );
                if let Err(error) = copied {
                    tracing::warn!(%error, "can't copy: enable the clipboard module or install wl-copy");
                }
            }
            Err(error) => tracing::warn!(%error, "the clipboard module couldn't copy it"),
        }
    });
}

/// `zones` with `zone` added or taken off, for the `zones` setting.
fn changed_zones(zones: &[String], add: bool, zone: &str) -> Result<Vec<String>, String> {
    let zone = zone.trim();
    if zone.is_empty() {
        return Err("which time zone? Like Europe/Paris".to_owned());
    }
    let mut zones = zones.to_vec();
    let at = zones.iter().position(|known| known == zone);
    match (add, at) {
        (true, Some(_)) => return Err(format!("{zone} is on the World tab already")),
        (true, None) if zones.len() >= MOST_ZONES => {
            return Err(format!("the World tab shows up to {MOST_ZONES} zones"));
        }
        (true, None) => zones.push(zone.to_owned()),
        (false, Some(at)) => {
            zones.remove(at);
        }
        (false, None) => return Err(format!("{zone} isn't on the World tab")),
    }
    Ok(zones)
}

/// A reminder's time here, like `2026-10-21 20:00`.
fn when(at: i64) -> String {
    local::from_epoch(at).map_or_else(
        || at.to_string(),
        |local| format!("{} {:02}:{:02}", local.date, local.hour, local.minute),
    )
}

/// What `reminders` prints: one line each, with its number.
fn listing<'a>(reminders: impl Iterator<Item = &'a Reminder>, now: i64) -> String {
    let lines: Vec<String> = reminders
        .map(|reminder| {
            let state = if reminder.done {
                " (done)".to_owned()
            } else if reminder.due(now) {
                " (due)".to_owned()
            } else if let Some(until) = reminder.snoozed {
                format!(" (snoozed to {})", when(until))
            } else {
                String::new()
            };
            format!(
                "{}  {}  {}{state}",
                reminder.id,
                when(reminder.at),
                reminder.text
            )
        })
        .collect();
    if lines.is_empty() {
        "no reminders".to_owned()
    } else {
        lines.join("\n")
    }
}

struct State {
    settings: Settings,
    reminders: Reminders,
    /// Where the reminders are kept; `None` without a home, when they last
    /// only as long as mochid.
    path: Option<PathBuf>,
    /// The stopwatch's past runs, and where they're kept, like the
    /// reminders.
    runs: Runs,
    runs_path: Option<PathBuf>,
    stopwatch: Stopwatch,
    /// The session directory, where the stopwatch is kept.
    session: PathBuf,
    zones: Offsets,
    /// The panel, while it's open.
    panel: Option<ActivityId>,
    /// The notices of due reminders on screen or waiting, by reminder.
    notices: BTreeMap<u64, ActivityId>,
    /// The reminders that came up since mochid started, so one whose
    /// notice went by unanswered doesn't come up again until a restart.
    told: BTreeSet<u64>,
}

impl State {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let now = seconds();
        let id = command
            .args
            .int("id")
            .map(|id| u64::try_from(id).map_err(|_| format!("there's no reminder {id}")));
        let result = match command.action.as_str() {
            "toggle" if self.panel.is_some() => {
                self.close(ctx);
                Ok(None)
            }
            "toggle" | "open" => {
                let tab = command
                    .args
                    .str("tab")
                    .map_or_else(|| tab_name(self.settings.tab), str::to_owned);
                self.open(ctx, &tab);
                Ok(None)
            }
            "close" => {
                self.close(ctx);
                Ok(None)
            }
            "remind" => self.remind(&command, now).map(Some),
            "reminders" => match command.args.str("date") {
                None => Ok(Some(listing(self.reminders.all().iter(), now))),
                Some(text) => today(now).and_then(|today| {
                    let date = local::parse_date(text, today)?;
                    let on_day = self.reminders.all().iter().filter(move |reminder| {
                        local::from_epoch(reminder.at).is_some_and(|local| local.date == date)
                    });
                    Ok(Some(listing(on_day, now)))
                }),
            },
            "delete" => id.unwrap_or(Err("which reminder?".into())).and_then(|id| {
                self.reminders.delete(id)?;
                self.close_notice(ctx, id);
                Ok(None)
            }),
            "done" => id.unwrap_or(Err("which reminder?".into())).and_then(|id| {
                self.reminders.done(id)?;
                self.close_notice(ctx, id);
                Ok(None)
            }),
            "snooze" => id.unwrap_or(Err("which reminder?".into())).and_then(|id| {
                let minutes = command.args.int("minutes").unwrap_or(SNOOZE);
                if !(1..=24 * 60).contains(&minutes) {
                    return Err(format!("minutes go from 1 to 1440, not {minutes}"));
                }
                self.reminders.snooze(id, now + minutes * 60)?;
                self.close_notice(ctx, id);
                // It comes up again when the snooze ends.
                self.told.remove(&id);
                Ok(None)
            }),
            "stopwatch" => self.stopwatch(command.args.str("what").unwrap_or_default()),
            "runs" => Ok(Some(self.runs.listing(self.settings.precision, when))),
            "copy-run" => self.run_text(command.args.int("run")).map(|text| {
                copy(ctx, text);
                None
            }),
            "forget-run" => self.runs.forget(command.args.int("run")).map(|()| None),
            "add-zone" | "remove-zone" => {
                let add = command.action == "add-zone";
                let zone = command.args.str("zone").unwrap_or_default();
                match changed_zones(&self.settings.zones, add, zone) {
                    Ok(zones) => {
                        // Answers once the setting is kept.
                        let set = ctx.settings_op(SettingsOp::Set {
                            path: "config.module.clock.zones".into(),
                            value: json!(zones),
                        });
                        self.settings.zones = zones;
                        self.publish(ctx);
                        tokio::spawn(async move { command.reply(set.await.map(drop)) });
                        return;
                    }
                    Err(error) => Err(error),
                }
            }
            "credit" => mochi_core::process::spawn_detached(
                &mochi_core::process::in_app_scope(&["xdg-open".into(), CREDIT.into()]),
                None,
            )
            .map(|()| None),
            other => Err(format!("clock has no action {other}")),
        };
        let changed = !matches!(
            command.action.as_str(),
            "reminders" | "toggle" | "open" | "close" | "runs" | "copy-run" | "credit"
        ) && result.is_ok();
        if changed {
            self.save(ctx);
        }
        match result {
            Ok(Some(output)) => command.answer(Ok(output)),
            Ok(None) => command.reply(Ok(())),
            Err(error) => command.reply(Err(error)),
        }
    }

    fn remind(&mut self, command: &ModuleCommand, now: i64) -> Result<String, String> {
        let date = local::parse_date(command.args.str("date").unwrap_or_default(), today(now)?)?;
        let (hour, minute) = local::parse_time(command.args.str("time").unwrap_or_default())?;
        let at = local::to_epoch(date, hour, minute).ok_or("that time doesn't exist here")?;
        let id = self
            .reminders
            .add(at, command.args.str("text").unwrap_or_default(), now)?;
        Ok(format!("reminder {id} at {}", when(at)))
    }

    fn stopwatch(&mut self, what: &str) -> Result<Option<String>, String> {
        let now = millis();
        match what {
            "start" => self.stopwatch.start(now),
            "pause" => self.stopwatch.pause(now),
            "toggle" => self.stopwatch.toggle(now),
            "lap" => self.stopwatch.lap(now)?,
            "reset" => {
                if let Some(run) = self.stopwatch.reset(now, seconds())
                    && self.settings.history > 0
                {
                    self.runs.add(run, self.settings.history as usize);
                }
            }
            "status" => {
                return Ok(Some(self.stopwatch.status(now, self.settings.precision)));
            }
            other => return Err(format!("the stopwatch can't {other}")),
        }
        Ok(None)
    }

    /// The run `number` as it's copied, or the one going without a number
    /// or with 0.
    fn run_text(&self, number: Option<i64>) -> Result<String, String> {
        let precision = self.settings.precision;
        match number.filter(|number| *number != 0) {
            None => {
                let total = self.stopwatch.elapsed(millis());
                if total == 0 {
                    return Err("the stopwatch hasn't run".to_owned());
                }
                Ok(stopwatch::report(
                    "Stopwatch",
                    total,
                    &self.stopwatch.laps,
                    precision,
                ))
            }
            Some(number) => {
                let run = self.runs.get(number)?;
                let title = format!("Stopwatch, {}", when(run.ended));
                Ok(stopwatch::report(
                    &title,
                    run.total_ms,
                    &run.laps,
                    precision,
                ))
            }
        }
    }

    /// After a change: keeps the reminders, the stopwatch and its runs, and
    /// publishes.
    fn save(&mut self, ctx: &ModuleCtx) {
        if let Some(path) = &self.path
            && let Err(error) = self.reminders.save(path)
        {
            tracing::warn!(%error, "the reminders");
        }
        save_stopwatch(&self.session, &self.stopwatch);
        self.save_runs();
        self.publish(ctx);
    }

    fn save_runs(&self) {
        if let Some(path) = &self.runs_path
            && let Err(error) = self.runs.save(path)
        {
            tracing::warn!(%error, "the stopwatch's runs");
        }
    }

    fn open(&mut self, ctx: &ModuleCtx, tab: &str) {
        let payload = json!({
            // Only the island on this monitor takes the keyboard.
            "output": ctx.compositor().state().focused_output,
            "tab": tab,
        });
        if let Some(id) = self.panel {
            ctx.update(id, payload);
            return;
        }
        ctx.close_other_panels();
        let spec = ActivitySpec::new("Panel")
            .key("panel")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(payload);
        self.panel = Some(ctx.present(spec));
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.panel.take() {
            ctx.withdraw(id);
        }
    }

    fn ended(&mut self, activity: ActivityId) {
        if self.panel == Some(activity) {
            self.panel = None;
        }
        self.notices.retain(|_, notice| *notice != activity);
    }

    /// Says on the island that each reminder that came due is due, once.
    fn tell_due(&mut self, ctx: &ModuleCtx) {
        let now = seconds();
        let due: Vec<Reminder> = self
            .reminders
            .due(now)
            .filter(|reminder| !self.told.contains(&reminder.id))
            .cloned()
            .collect();
        for reminder in due {
            tracing::info!(id = reminder.id, "a reminder came due");
            self.told.insert(reminder.id);
            let spec = ActivitySpec::new("Notice")
                .key(format!("reminder-{}", reminder.id))
                .priority(Priority::HIGH)
                .timeout(NOTICE)
                .payload(json!({
                    "id": reminder.id,
                    "text": reminder.text,
                    "at": reminder.at,
                    "hours": self.settings.hours,
                }));
            self.notices.insert(reminder.id, ctx.present(spec));
        }
    }

    fn close_notice(&mut self, ctx: &ModuleCtx, id: u64) {
        if let Some(notice) = self.notices.remove(&id) {
            ctx.withdraw(notice);
        }
    }

    /// How long until the next look at the clock: the next reminder, or a
    /// little while when that's further off, for a suspend.
    fn wait(&self, now: i64) -> Option<Duration> {
        let next = self.reminders.next_due(now)?;
        let left = Duration::from_secs(u64::try_from(next - now).unwrap_or(0));
        Some(left.min(LOOK))
    }

    /// What the views read: the settings, the zones' offsets, the
    /// reminders, the stopwatch and its past runs.
    fn publish(&self, ctx: &ModuleCtx) {
        let (offsets, unknown) = self.zones.fields();
        let settings = &self.settings;
        ctx.publish_state(json!({
            "tab": settings.tab,
            "hours": settings.hours,
            "first_day": settings.first_day,
            "seconds": settings.seconds,
            "day_progress": settings.day_progress,
            "shape": settings.shape,
            "precision": settings.precision,
            "history": settings.history,
            "world": settings.zones,
            "zones": offsets,
            "unknownZones": unknown,
            "reminders": self.reminders.all(),
            "stopwatch": self.stopwatch,
            "runs": self.runs,
        }));
    }
}

fn tab_name(tab: Tab) -> String {
    serde_json::to_value(tab)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "today".to_owned())
}

/// The date here at `now`.
fn today(now: i64) -> Result<local::Date, String> {
    local::from_epoch(now)
        .map(|local| local.date)
        .ok_or_else(|| "can't read the local time".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> mochi_core::toml::Table {
        mochi_core::toml::from_str(text).unwrap()
    }

    #[test]
    fn the_example_matches_the_settings() {
        mochi_core::examples::check_module::<Settings>("clock", include_str!("../settings.toml"));
    }

    #[test]
    fn every_setting_applies_live() {
        let schema = mochi_core::options::schema_of::<Settings>();
        let mut keys: Vec<&str> = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let mut live = LIVE.to_vec();
        live.sort_unstable();
        assert_eq!(keys, live);
    }

    #[test]
    fn reads_the_settings() {
        let settings = Settings::load(&table(
            "tab = \"world\"\nhours = \"12\"\nzones = [\" Asia/Tokyo \", \"\"]",
        ))
        .unwrap();
        assert_eq!(settings.tab, Tab::World);
        assert_eq!(settings.hours, Hours::Twelve);
        assert_eq!(settings.zones, ["Asia/Tokyo"]);
        assert_eq!(tab_name(settings.tab), "world");
        assert!(Settings::load(&table("tab = \"alarm\"")).is_err());
        assert!(Settings::load(&table("hours = \"13\"")).is_err());
        let many: Vec<String> = (0..9).map(|n| format!("\"Etc/GMT+{n}\"")).collect();
        assert!(Settings::load(&table(&format!("zones = [{}]", many.join(",")))).is_err());
        assert!(Settings::load(&table("history = 50")).is_ok());
        assert!(Settings::load(&table("history = 51")).is_err());
        assert!(Settings::load(&table("shape = \"hexagon\"")).is_ok());
        assert!(Settings::load(&table("shape = \"star\"")).is_err());
        // Every tab the setting takes, the open action takes.
        for tab in [
            Tab::Today,
            Tab::Calendar,
            Tab::Timer,
            Tab::Stopwatch,
            Tab::World,
        ] {
            assert!(TABS.contains(&tab_name(tab).as_str()));
        }
    }

    #[test]
    fn adds_and_takes_off_zones() {
        let zones = vec!["Europe/London".to_owned()];
        assert_eq!(
            changed_zones(&zones, true, " Asia/Tokyo ").unwrap(),
            ["Europe/London", "Asia/Tokyo"]
        );
        assert!(changed_zones(&zones, true, "Europe/London").is_err());
        assert!(changed_zones(&zones, true, "  ").is_err());
        assert!(changed_zones(&zones, false, "Asia/Tokyo").is_err());
        assert!(
            changed_zones(&zones, false, "Europe/London")
                .unwrap()
                .is_empty()
        );
        let full: Vec<String> = (0..MOST_ZONES).map(|n| format!("Etc/GMT+{n}")).collect();
        assert!(changed_zones(&full, true, "Asia/Tokyo").is_err());
    }

    #[test]
    fn lists_reminders_with_their_state() {
        let now = 1_790_000_000;
        let mut reminders = Reminders::default();
        assert_eq!(listing(reminders.all().iter(), now), "no reminders");
        reminders.add(now, "Stretch", now).unwrap();
        reminders.add(now + 3600, "Call Ana", now).unwrap();
        reminders.add(now + 7200, "Done already", now).unwrap();
        reminders.done(2).unwrap();
        let text = listing(reminders.all().iter(), now);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("0  ") && lines[0].ends_with("Stretch (due)"));
        assert!(lines[1].ends_with("Call Ana"));
        assert!(lines[2].ends_with("Done already (done)"));
        assert_eq!(when(now).len(), "2026-10-21 20:00".len());
    }
}

//! A focus timer: `start` counts down a focus session in a bubble by the
//! island, and when it ends the island says so and offers the break; when
//! the break ends, it offers the next session. Every few sessions the break
//! is a long one. The control center gets a card with the time left and
//! buttons.
//!
//! A click on the bubble pauses or resumes it. The views count down
//! themselves from when the phase ends, so the daemon only speaks when
//! something changes. The timer is saved in the session directory, so it
//! carries on when mochid restarts, and its settings apply without a
//! restart, so changing them doesn't stop what runs.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.timer]
//! focus_minutes = 25
//! break_minutes = 5
//! long_break_minutes = 15
//! sessions_before_long_break = 4   # 0 never takes a long break
//! auto_break = false               # start the break without asking
//! ```

mod clock;
mod tour;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::clock::{Clock, Ended, Phase};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The file in the session directory that keeps the timer across restarts.
const FILE: &str = "timer.json";
/// How long the notice at the end of a phase stays on screen.
const NOTICE: Duration = Duration::from_secs(30);
/// The longest phase an action takes, a day.
const MOST_MINUTES: i64 = 24 * 60;
/// Every setting applies to the next phase, so none needs a restart.
const LIVE: [&str; 5] = [
    "focus_minutes",
    "break_minutes",
    "long_break_minutes",
    "sessions_before_long_break",
    "auto_break",
];

#[derive(Debug, Default)]
pub struct Timer;

#[derive(Debug, Clone, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// Minutes of focus that `start` counts down without an argument.
    #[schemars(range(min = 1, max = 1440))]
    focus_minutes: u64,
    /// Minutes of rest after a focus session.
    #[schemars(range(min = 1, max = 1440))]
    break_minutes: u64,
    /// Minutes of rest after every `sessions_before_long_break` focus
    /// sessions.
    #[schemars(range(min = 1, max = 1440))]
    long_break_minutes: u64,
    /// How many focus sessions come before a long break. 0 never takes one.
    #[schemars(range(min = 0, max = 12))]
    sessions_before_long_break: u32,
    /// Starts the break by itself when focus ends, instead of asking.
    auto_break: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            break_minutes: 5,
            long_break_minutes: 15,
            sessions_before_long_break: 4,
            auto_break: false,
        }
    }
}

impl Module for Timer {
    fn id(&self) -> &'static str {
        "timer"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let minutes = |help: &str| ArgSpec::int("minutes", help).optional();
        vec![
            ActionSpec::new("start", "Start a focus session")
                .arg(minutes("How long, instead of focus_minutes")),
            ActionSpec::new("break", "Start a break")
                .arg(minutes("How long, instead of the break's length")),
            ActionSpec::new("pause", "Pause the timer"),
            ActionSpec::new("resume", "Carry on from where it paused"),
            ActionSpec::new(
                "toggle",
                "Pause or resume, or start a focus session when nothing runs",
            ),
            ActionSpec::new("stop", "Stop the timer"),
            ActionSpec::new("status", "Print what the timer is doing"),
        ]
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("control-center", "card", "timer", "Card", "Focus timer")
                .icon("timer")
                .order(14)
                .options(json!({ "span": 1, "rows": 1 })),
            ContributionSpec::new("widgets", "widget", "focus", "Widget", "Focus timer")
                .icon("timer")
                .options(json!({
                    "size": [11, 13],
                    "min": [9, 11],
                    "max": [24, 28],
                    "category": "Clock",
                    "description": "The time left in a ring, or a button to start",
                })),
        ];
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

    fn live_settings(&self) -> &'static [&'static str] {
        &LIVE
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let dir = ctx.session_dir().to_owned();
            let mut state = State {
                clock: load(&dir),
                settings,
                dir,
                bubble: None,
                notice: None,
            };
            // A timer from the last run shows again; one that ran out
            // meanwhile ends on the first tick.
            state.changed(&ctx, true);
            loop {
                let wait = state.clock.wait(now());
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::BubbleClicked(_)) => {
                            state.clock.toggle(now(), &state.settings);
                            state.close_notice(&ctx);
                            state.changed(&ctx, false);
                        }
                        Some(ModuleEvent::Ended { activity, .. }) if state.notice == Some(activity) => {
                            state.notice = None;
                        }
                        Some(ModuleEvent::Reconfigured(table)) => {
                            match mochi_core::settings::<Settings>(&table) {
                                Ok(settings) => {
                                    state.settings = settings;
                                    state.publish(&ctx);
                                }
                                Err(error) => tracing::warn!(%error, "the timer's new settings"),
                            }
                        }
                        Some(_) => {}
                    },
                    () = sleep(wait) => {
                        if let Some(ended) = state.clock.tick(now(), &state.settings) {
                            state.tell(&ctx, ended);
                            state.changed(&ctx, true);
                        }
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

/// The wall clock, in milliseconds since the epoch.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

fn load(dir: &Path) -> Clock {
    let Ok(text) = std::fs::read_to_string(dir.join(FILE)) else {
        return Clock::default();
    };
    serde_json::from_str(&text)
        .inspect_err(|error| tracing::warn!(%error, "ignoring the saved timer"))
        .unwrap_or_default()
}

fn save(dir: &Path, clock: &Clock) {
    let written = std::fs::create_dir_all(dir).and_then(|()| {
        let temporary = dir.join(format!("{FILE}.new"));
        std::fs::write(&temporary, serde_json::to_vec(clock).unwrap_or_default())?;
        std::fs::rename(temporary, dir.join(FILE))
    });
    if let Err(error) = written {
        tracing::warn!(%error, "can't save the timer");
    }
}

struct State {
    settings: Settings,
    clock: Clock,
    /// The session directory, where the timer is saved.
    dir: PathBuf,
    bubble: Option<BubbleId>,
    /// The notice that a phase ended, while it shows.
    notice: Option<ActivityId>,
}

impl State {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let minutes = match command.args.int("minutes") {
            Some(minutes) if !(1..=MOST_MINUTES).contains(&minutes) => {
                command.reply(Err(format!(
                    "minutes go from 1 to {MOST_MINUTES}, not {minutes}"
                )));
                return;
            }
            minutes => minutes.map(|minutes| minutes as u64),
        };
        let now = now();
        // A new phase is news for the bubble; a pause isn't.
        let (result, news) = match command.action.as_str() {
            "start" => {
                let minutes = minutes.unwrap_or(self.settings.focus_minutes);
                self.clock.start(Phase::Focus, minutes, now);
                (Ok(()), true)
            }
            "break" => {
                let minutes = minutes.unwrap_or(self.clock.next_break(&self.settings).0);
                self.clock.start(Phase::Break, minutes, now);
                (Ok(()), true)
            }
            "pause" => (self.clock.pause(now), false),
            "resume" => (self.clock.resume(now), false),
            "toggle" => {
                let news = self.clock.countdown.is_none();
                self.clock.toggle(now, &self.settings);
                (Ok(()), news)
            }
            "stop" => {
                self.clock.stop();
                (Ok(()), false)
            }
            "status" => {
                command.answer(Ok(self.clock.status(now)));
                return;
            }
            other => (Err(format!("timer has no action {other}")), false),
        };
        if result.is_ok() {
            // Whatever the notice offered, the user has moved on.
            self.close_notice(ctx);
            self.changed(ctx, news);
        }
        command.reply(result);
    }

    /// Says on the island that a phase ended, and what comes next.
    fn tell(&mut self, ctx: &ModuleCtx, ended: Ended) {
        tracing::info!(finished = ended.finished.name(), "a phase ended");
        let spec = ActivitySpec::new("Notice")
            .key("notice")
            .priority(Priority::HIGH)
            .timeout(NOTICE)
            .payload(json!({
                "finished": ended.finished.name(),
                "next": ended.next.name(),
                "minutes": ended.minutes,
                "long": ended.long,
                "started": ended.started,
                "sessions": self.clock.sessions,
            }));
        self.notice = Some(ctx.present(spec));
    }

    fn close_notice(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.notice.take() {
            ctx.withdraw(id);
        }
    }

    /// After the timer changed: saves it, shows the bubble or updates it
    /// in place, and publishes the state. A new phase is `news`.
    fn changed(&mut self, ctx: &ModuleCtx, news: bool) {
        save(&self.dir, &self.clock);
        let payload = self.payload(now());
        match (&self.clock.countdown, self.bubble) {
            (Some(_), Some(id)) if !news => ctx.update_bubble(id, payload),
            (Some(_), _) => {
                let spec = BubbleSpec::new("Bubble")
                    .key("countdown")
                    .wide("Wide")
                    .area(Area::CenterRight)
                    .payload(payload)
                    .news();
                self.bubble = Some(ctx.show_bubble(spec));
            }
            (None, Some(id)) => {
                ctx.hide_bubble(id);
                self.bubble = None;
            }
            (None, None) => {}
        }
        self.publish(ctx);
    }

    /// What the bubble and the card show. The views count down from
    /// `ends_ms` while it runs.
    fn payload(&self, now: u64) -> Value {
        let mut payload = json!({
            "phase": "idle",
            "sessions": self.clock.sessions,
            "focus_minutes": self.settings.focus_minutes,
        });
        if let Some(countdown) = &self.clock.countdown {
            payload["phase"] = json!(countdown.phase.name());
            payload["paused"] = json!(countdown.paused());
            payload["ends_ms"] = json!(countdown.ends_ms);
            payload["left_ms"] = json!(countdown.left(now));
            payload["total_ms"] = json!(countdown.total_ms);
        }
        payload
    }

    fn publish(&self, ctx: &ModuleCtx) {
        ctx.publish_state(self.payload(now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_matches_the_settings() {
        mochi_core::examples::check_module::<Settings>("timer", include_str!("../settings.toml"));
    }

    #[test]
    fn every_setting_applies_live() {
        // The schema's properties are the settings' keys.
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
    fn the_timer_survives_a_save() {
        let dir = std::env::temp_dir().join(format!("mochi-timer-{}", std::process::id()));
        let mut clock = Clock {
            sessions: 2,
            ..Clock::default()
        };
        clock.start(Phase::Break, 5, now());
        save(&dir, &clock);
        assert_eq!(load(&dir), clock);
        std::fs::write(dir.join(FILE), "not json").unwrap();
        assert_eq!(load(&dir), Clock::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! A focus timer and custom timers.
//!
//! `start` counts down a focus session in a bubble by the island, and when
//! it ends the island says so and offers the break; when the break ends,
//! it offers the next session. Every few sessions the break is a long one.
//! The control center gets a card with the time left and buttons.
//!
//! Custom timers run beside it, like a kitchen timer's: `add 15m Tea`
//! starts one, with a length typed the way people write it (see
//! [`duration`]) and an optional label. Several run at once, each with its
//! own bubble, pause and "+1 min". When one runs out, the island says so,
//! the alarm plays and the music pauses, through the media module. The
//! launcher starts them too, after `:t `, and starts or pauses the clock
//! module's stopwatch with `:t sw`.
//!
//! A click on a bubble pauses or resumes its timer. The views count down
//! themselves from when the time runs out, so the daemon only speaks when
//! something changes. The timers are saved in the session directory, so
//! they carry on when mochid restarts, and the settings apply without a
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
//! sound = true                     # the alarm when a custom timer ends
//! sound_file = ""                  # empty for alarm-clock-elapsed
//! volume = 80
//! sound_command = []               # like ["mpv", "--really-quiet"]
//! focus_sound = false              # the alarm for focus and breaks too
//! pause_media = true
//! timer_bubbles = "each"           # "each", "soonest" or "off"
//! extend_seconds = 60              # what "+1 min" adds
//! presets = ["3m", "5m", "10m", "15m", "30m"]
//! ```
//!
//! Custom timers, their alarm and the launcher's `:t` come from
//! [mochi-clock](https://github.com/Xonex5/mochi-clock) by Xonex5.

mod clock;
mod duration;
mod timers;
mod tour;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use include_dir::{Dir, include_dir};
use mochi_core::sound::{self, Sound, Speaker};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    CallError, ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent,
    Priority,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::clock::{Clock, Ended, Phase};
use crate::timers::{Custom, Timers, Which};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The file in the session directory that keeps the timers across
/// restarts.
const FILE: &str = "timer.json";
/// How long the notice at the end of a phase or a timer stays on screen.
const NOTICE: Duration = Duration::from_secs(30);
/// The longest phase an action takes, a day.
const MOST_MINUTES: i64 = 24 * 60;
/// The alarm without a `sound_file`, from the sound theme.
const ALARM: &str = "alarm-clock-elapsed";
/// The sound theme the alarm comes from.
const THEME: &str = "freedesktop";
/// Every setting applies at once or to the next timer, so none needs a
/// restart.
const LIVE: [&str; 14] = [
    "focus_minutes",
    "break_minutes",
    "long_break_minutes",
    "sessions_before_long_break",
    "auto_break",
    "sound",
    "sound_file",
    "volume",
    "sound_command",
    "focus_sound",
    "pause_media",
    "timer_bubbles",
    "extend_seconds",
    "presets",
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
    /// Plays the alarm when a custom timer ends.
    sound: bool,
    /// A sound file for the alarm, like ~/sounds/bell.oga. Empty plays
    /// alarm-clock-elapsed from the sound theme.
    sound_file: String,
    /// How loud the alarm plays, from 0 to 100.
    #[schemars(range(min = 0, max = 100))]
    volume: u8,
    /// What plays the alarm instead of pw-play or paplay, with any options
    /// of your own; the file goes last, and volume doesn't apply. Like
    /// ["mpv", "--really-quiet"].
    sound_command: Vec<String>,
    /// Plays the alarm when a focus session or a break ends too.
    focus_sound: bool,
    /// Pauses the music or video playing when a custom timer ends, through
    /// the media module.
    pause_media: bool,
    /// The bubbles of custom timers: one for each, side by side, one for
    /// the timer that ends first, or none.
    timer_bubbles: TimerBubbles,
    /// The seconds "+1 min" adds to a custom timer.
    #[schemars(range(min = 1, max = 3600))]
    extend_seconds: u64,
    /// The lengths the clock panel's Timer tab and the launcher offer,
    /// like "5m" or "1h 30m".
    presets: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            break_minutes: 5,
            long_break_minutes: 15,
            sessions_before_long_break: 4,
            auto_break: false,
            sound: true,
            sound_file: String::new(),
            volume: 80,
            sound_command: Vec::new(),
            focus_sound: false,
            pause_media: true,
            timer_bubbles: TimerBubbles::Each,
            extend_seconds: 60,
            presets: ["3m", "5m", "10m", "15m", "30m"]
                .map(str::to_owned)
                .to_vec(),
        }
    }
}

impl Settings {
    /// The settings, with the alarm's file and the presets checked.
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if !settings.sound_file.is_empty() && settings.alarm().is_none() {
            return Err(format!(
                "sound_file {:?} isn't a whole path, like ~/sounds/bell.oga",
                settings.sound_file
            ));
        }
        for preset in &settings.presets {
            duration::parse(preset).map_err(|error| format!("presets: {error}"))?;
        }
        Ok(settings)
    }

    /// The alarm: `sound_file`, or the sound theme's.
    fn alarm(&self) -> Option<Sound> {
        if self.sound_file.is_empty() {
            return Sound::name(ALARM);
        }
        let home = std::env::var("HOME").unwrap_or_default();
        match self.sound_file.strip_prefix("~/") {
            Some(rest) if !home.is_empty() => Sound::file(&format!("{home}/{rest}")),
            _ => Sound::file(&self.sound_file),
        }
    }

    fn speaker(&self) -> Speaker {
        Speaker::new(self.sound_command.clone(), THEME.to_owned())
            .with_volume(f64::from(self.volume) / 100.0)
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
enum TimerBubbles {
    /// One bubble for each timer, side by side.
    #[default]
    Each,
    /// One bubble, for the timer that ends first.
    Soonest,
    /// No bubbles; the clock panel and the launcher still show them.
    Off,
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
        let which = |what: &str| {
            ArgSpec::string(
                "id",
                format!("A custom timer's number from list, or all; without one, {what}"),
            )
            .optional()
        };
        vec![
            ActionSpec::new("start", "Start a focus session")
                .arg(minutes("How long, instead of focus_minutes")),
            ActionSpec::new("break", "Start a break")
                .arg(minutes("How long, instead of the break's length")),
            ActionSpec::new("pause", "Pause the focus session or a custom timer")
                .arg(which("the focus session")),
            ActionSpec::new("resume", "Carry on from where it paused")
                .arg(which("the focus session")),
            ActionSpec::new(
                "toggle",
                "Pause or resume, or start a focus session when nothing runs",
            )
            .arg(which("the focus session")),
            ActionSpec::new("stop", "Stop the focus session or a custom timer")
                .arg(which("the focus session")),
            ActionSpec::new("status", "Print what the focus timer is doing"),
            ActionSpec::new("add", "Start a custom timer").arg(
                ArgSpec::string(
                    "timer",
                    "A length and a label, like 15m Tea, 90s, 1h 30m or 10:00",
                )
                .rest(),
            ),
            ActionSpec::new("extend", "Add time to a custom timer")
                .arg(ArgSpec::string("id", "Its number from list, or all"))
                .arg(
                    ArgSpec::string("length", "How much, like 5m; extend_seconds without")
                        .optional()
                        .rest(),
                ),
            ActionSpec::new("list", "Print the custom timers, a line each"),
            ActionSpec::new("test-sound", "Play the alarm, to hear how it sounds"),
            ActionSpec::new(
                "search",
                "Answer with launcher results for a length or sw; the launcher sends this",
            )
            .arg(
                ArgSpec::string("query", "Like 10m Pizza, or sw for the stopwatch")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new("pick-result", "Do what a launcher result offered")
                .arg(ArgSpec::string("id", "The result's id").rest()),
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
            // No view: the launcher shows what `search` answers. The space
            // keeps emoji like :tea with the emoji module's `:`.
            ContributionSpec::new("launcher", "provider", "timer", "", "Timers")
                .icon("hourglass_top")
                .options(json!({ "prefix": ":t ", "search": "search", "pick": "pick-result" })),
            // The alarm's test button and the credit, on the timer's page in
            // the settings.
            ContributionSpec::new("settings", "section", "alarm", "Alarm", "Alarm"),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn needs(&self, table: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        let settings = Settings::load(table).unwrap_or_default();
        // canberra-gtk-play isn't one: without it, Mochi finds the sound
        // theme's files itself.
        if !settings.sound && !settings.focus_sound {
            return Vec::new();
        }
        vec![match settings.sound_command.first() {
            Some(program) => mochi_core::Need::new(program, "The timer's alarm"),
            None => mochi_core::Need::new(
                sound::player().unwrap_or(sound::PLAYERS[0]),
                "The timer's alarm, with pw-play or paplay",
            ),
        }]
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

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let table: mochi_core::toml::Table = ctx.settings()?;
            let settings = Settings::load(&table)?;
            let dir = ctx.session_dir().to_owned();
            let saved = load(&dir);
            let mut state = State {
                speaker: settings.speaker(),
                settings,
                clock: saved.clock,
                timers: saved.timers,
                dir,
                bubble: None,
                bubbles: BTreeMap::new(),
                fresh: None,
                notice: None,
                rang: Vec::new(),
                stopwatch: Value::Null,
            };
            // For the launcher's stopwatch results.
            ctx.watch_state("clock");
            // Timers from the last run show again; those that ran out
            // meanwhile end on the first tick.
            state.changed(&ctx, true);
            loop {
                let now = now();
                let wait = match (state.clock.wait(now), state.timers.wait(now)) {
                    (Some(focus), Some(custom)) => Some(focus.min(custom)),
                    (focus, custom) => focus.or(custom),
                };
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::BubbleClicked(id)) => state.clicked(&ctx, id),
                        Some(ModuleEvent::Ended { activity, .. }) => {
                            if state.notice == Some(activity) {
                                state.notice = None;
                            }
                            state.rang.retain(|notice| *notice != activity);
                        }
                        Some(ModuleEvent::State { module, state: clock }) if module == "clock" => {
                            state.stopwatch = clock["stopwatch"].clone();
                        }
                        Some(ModuleEvent::Reconfigured(table)) => match Settings::load(&table) {
                            Ok(settings) => {
                                state.speaker = settings.speaker();
                                state.settings = settings;
                                state.changed(&ctx, false);
                            }
                            Err(error) => tracing::warn!(%error, "the timer's new settings"),
                        },
                        Some(_) => {}
                    },
                    () = sleep(wait) => state.tick(&ctx),
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

/// What the session directory keeps: the focus timer, as it was before
/// custom timers, and the custom timers beside it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Saved {
    #[serde(flatten)]
    clock: Clock,
    #[serde(default)]
    timers: Timers,
}

fn load(dir: &Path) -> Saved {
    let Ok(text) = std::fs::read_to_string(dir.join(FILE)) else {
        return Saved::default();
    };
    serde_json::from_str(&text)
        .inspect_err(|error| tracing::warn!(%error, "ignoring the saved timer"))
        .unwrap_or_default()
}

fn save(dir: &Path, saved: &Saved) {
    let written = std::fs::create_dir_all(dir).and_then(|()| {
        let temporary = dir.join(format!("{FILE}.new"));
        std::fs::write(&temporary, serde_json::to_vec(saved).unwrap_or_default())?;
        std::fs::rename(temporary, dir.join(FILE))
    });
    if let Err(error) = written {
        tracing::warn!(%error, "can't save the timer");
    }
}

struct State {
    settings: Settings,
    clock: Clock,
    timers: Timers,
    /// The session directory, where the timers are saved.
    dir: PathBuf,
    /// The focus session's bubble.
    bubble: Option<BubbleId>,
    /// The custom timers' bubbles, by timer number; 0 for the one bubble
    /// with `timer_bubbles = "soonest"`.
    bubbles: BTreeMap<u32, BubbleId>,
    /// A custom timer that just started, whose bubble is news.
    fresh: Option<u32>,
    /// The notice that a phase ended, while it shows.
    notice: Option<ActivityId>,
    /// The notices that custom timers ran out, while they show.
    rang: Vec<ActivityId>,
    speaker: Speaker,
    /// The clock module's stopwatch, from its state; null without it.
    stopwatch: Value,
}

impl State {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let now = now();
        let which = match command.args.str("id").map(Which::parse) {
            Some(Err(error)) if command.action != "pick-result" => {
                command.reply(Err(error));
                return;
            }
            Some(Ok(which)) => Some(which),
            _ => None,
        };
        let result = match (command.action.as_str(), which) {
            ("add", _) => {
                let text = command.args.str("timer").unwrap_or_default().to_owned();
                match self.add(&text, now) {
                    Ok(id) => {
                        self.changed(ctx, false);
                        command.answer(Ok(format!("timer {id}")));
                    }
                    Err(error) => command.answer(Err(error)),
                }
                return;
            }
            ("pause", Some(which)) => self.timers.pause(which, now),
            ("resume", Some(which)) => self.timers.resume(which, now),
            ("stop", Some(which)) => self.timers.stop(which),
            ("toggle", Some(Which::One(id))) => self.timers.toggle(id, now),
            ("toggle", Some(Which::All)) => Err("toggle takes one timer's number".to_owned()),
            ("extend", Some(which)) => self.extend(which, command.args.str("length")),
            ("list", _) => {
                command.answer(Ok(self.timers.status(now)));
                return;
            }
            ("test-sound", _) => {
                let result = self.alarm();
                command.reply(result);
                return;
            }
            ("search", _) => {
                let lines: Vec<String> = self
                    .search(command.args.str("query").unwrap_or_default(), now)
                    .iter()
                    .map(Value::to_string)
                    .collect();
                command.answer(Ok(lines.join("\n")));
                return;
            }
            ("pick-result", _) => {
                let result = self.pick(ctx, command.args.str("id").unwrap_or_default(), now);
                command.reply(result);
                return;
            }
            _ => {
                self.focus(ctx, command);
                return;
            }
        };
        if result.is_ok() {
            self.changed(ctx, false);
        }
        command.reply(result);
    }

    /// The focus session's actions.
    fn focus(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
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

    /// Starts a custom timer from what was typed, like `15m Tea`.
    fn add(&mut self, text: &str, now: u64) -> Result<u32, String> {
        let parsed = duration::parse(text)?;
        let id = self.timers.add(parsed.ms, &parsed.label, now)?;
        tracing::info!(id, ms = parsed.ms, "a timer started");
        self.fresh = Some(id);
        Ok(id)
    }

    fn extend(&mut self, which: Which, length: Option<&str>) -> Result<(), String> {
        let ms = match length {
            Some(text) => duration::parse(text)?.ms,
            None => self.settings.extend_seconds.saturating_mul(1000),
        };
        self.timers.extend(which, ms)
    }

    /// A click on a bubble pauses or resumes its timer.
    fn clicked(&mut self, ctx: &ModuleCtx, bubble: BubbleId) {
        let now = now();
        if self.bubble == Some(bubble) {
            self.clock.toggle(now, &self.settings);
            self.close_notice(ctx);
        } else {
            let id = match self.bubbles.iter().find(|(_, shown)| **shown == bubble) {
                Some((0, _)) => self.timers.soonest(now).map(|timer| timer.id),
                Some((id, _)) => Some(*id),
                None => None,
            };
            if let Some(id) = id {
                let _ = self.timers.toggle(id, now);
            }
        }
        self.changed(ctx, false);
    }

    /// Ends what ran out: a phase of the focus timer, custom timers.
    fn tick(&mut self, ctx: &ModuleCtx) {
        let now = now();
        let ended = self.clock.tick(now, &self.settings);
        let rang = self.timers.tick(now);
        if let Some(ended) = ended {
            self.tell(ctx, ended);
        }
        if !rang.is_empty() {
            self.ring(ctx, &rang);
        }
        if ended.is_some() || !rang.is_empty() {
            self.changed(ctx, ended.is_some());
        }
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
        if self.settings.focus_sound {
            self.ring_alarm();
        }
    }

    /// Custom timers ran out: a notice for each, the alarm once, and the
    /// music paused.
    fn ring(&mut self, ctx: &ModuleCtx, ended: &[Custom]) {
        for timer in ended {
            tracing::info!(id = timer.id, "a timer ran out");
            let spec = ActivitySpec::new("Done")
                .key(format!("done-{}", timer.id))
                .priority(Priority::HIGH)
                .timeout(NOTICE)
                .payload(json!({
                    "label": timer.label,
                    "name": timer.name(),
                    "length": duration::words(timer.total_ms),
                    // What Again and +1 min type.
                    "again": format!("{} {}", duration::short(timer.total_ms), timer.label),
                    "more": format!("{} {}", duration::short(self.settings.extend_seconds * 1000), timer.label),
                    "more_label": format!("+{}", duration::words(self.settings.extend_seconds * 1000)),
                }));
            self.rang.push(ctx.present(spec));
        }
        if self.settings.sound {
            self.ring_alarm();
        }
        if self.settings.pause_media {
            let pause = ctx.call("media", "pause", &[]);
            tokio::spawn(async move {
                match pause.await {
                    Ok(()) | Err(CallError::NotEnabled(_)) => {}
                    Err(error) => tracing::debug!(%error, "couldn't pause the media"),
                }
            });
        }
    }

    fn alarm(&mut self) -> Result<(), String> {
        if self.settings.volume == 0 {
            return Err("the alarm's volume is 0".to_owned());
        }
        let sound = self
            .settings
            .alarm()
            .ok_or_else(|| format!("can't play {:?}", self.settings.sound_file))?;
        if !self.speaker.play(&sound) {
            return Err(match sound {
                Sound::Name(_) => format!(
                    "nothing plays {ALARM}: it needs pw-play or paplay and the freedesktop \
                     sound theme, or canberra-gtk-play; or set sound_file"
                ),
                Sound::File(_) => "nothing plays the alarm: it needs pw-play or paplay, or \
                                   sound_command"
                    .to_owned(),
            });
        }
        Ok(())
    }

    /// The alarm at the end of a timer: a problem goes to the log, since
    /// nobody waits on an answer.
    fn ring_alarm(&mut self) {
        // Volume 0 is a quiet alarm on purpose.
        if self.settings.volume == 0 {
            return;
        }
        if let Err(error) = self.alarm() {
            tracing::warn!(%error, "the timer's alarm");
        }
    }

    fn close_notice(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.notice.take() {
            ctx.withdraw(id);
        }
    }

    /// After the timers changed: saves them, shows the bubbles or updates
    /// them in place, and publishes the state. A new phase is `news`.
    fn changed(&mut self, ctx: &ModuleCtx, news: bool) {
        let saved = Saved {
            clock: self.clock.clone(),
            timers: self.timers.clone(),
        };
        save(&self.dir, &saved);
        let now = now();
        let payload = self.focus_payload(now);
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
        self.sync_bubbles(ctx, now);
        self.publish(ctx);
    }

    /// The custom timers' bubbles, as `timer_bubbles` says: one each, one
    /// for the soonest, or none.
    fn sync_bubbles(&mut self, ctx: &ModuleCtx, now: u64) {
        let count = self.timers.list.len();
        let wanted: Vec<(u32, Value)> = match self.settings.timer_bubbles {
            TimerBubbles::Each => self
                .timers
                .list
                .iter()
                .map(|timer| (timer.id, timer_payload(timer, now, 1)))
                .collect(),
            TimerBubbles::Soonest => self
                .timers
                .soonest(now)
                .map(|timer| (0, timer_payload(timer, now, count)))
                .into_iter()
                .collect(),
            TimerBubbles::Off => Vec::new(),
        };
        let fresh = self.fresh.take();
        self.bubbles.retain(|key, bubble| {
            let keep = wanted.iter().any(|(wanted, _)| wanted == key);
            if !keep {
                ctx.hide_bubble(*bubble);
            }
            keep
        });
        for (key, payload) in wanted {
            let news = fresh.is_some_and(|fresh| key == 0 || key == fresh);
            match self.bubbles.get(&key) {
                Some(bubble) if !news => ctx.update_bubble(*bubble, payload),
                _ => {
                    // Side by side in one pill, in the order they started,
                    // after the focus session's.
                    let spec = BubbleSpec::new("Bubble")
                        .key(format!("timer-{key}"))
                        .wide("Wide")
                        .area(Area::CenterRight)
                        .group("timers")
                        .order(i32::try_from(key).unwrap_or(i32::MAX).saturating_add(1))
                        .payload(payload)
                        .news();
                    self.bubbles.insert(key, ctx.show_bubble(spec));
                }
            }
        }
    }

    /// What the focus bubble and the card show. The views count down from
    /// `ends_ms` while it runs.
    fn focus_payload(&self, now: u64) -> Value {
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

    /// The focus session's payload, with the custom timers and what the
    /// views offer to start them.
    fn publish(&self, ctx: &ModuleCtx) {
        let now = now();
        let mut state = self.focus_payload(now);
        state["timers"] = self
            .timers
            .list
            .iter()
            .map(|timer| timer_payload(timer, now, 1))
            .collect();
        state["presets"] = self
            .settings
            .presets
            .iter()
            .filter_map(|preset| {
                let parsed = duration::parse(preset).ok()?;
                Some(json!({ "text": preset, "label": duration::words(parsed.ms) }))
            })
            .collect();
        state["more_label"] = json!(format!(
            "+{}",
            duration::words(self.settings.extend_seconds * 1000)
        ));
        // For the test button on the settings page.
        state["alarm"] = json!({
            "sound": self.settings.sound,
            "focus_sound": self.settings.focus_sound,
            "volume": self.settings.volume,
        });
        ctx.publish_state(state);
    }

    /// The launcher's results for what follows `:t `.
    fn search(&self, query: &str, now: u64) -> Vec<Value> {
        let query = query.trim();
        let lower = query.to_lowercase();
        let mut results = Vec::new();
        if query.is_empty() {
            for timer in &self.timers.list {
                let paused = timer.paused();
                results.push(json!({
                    "title": format!("{} {}", if paused { "Resume" } else { "Pause" }, timer.name()),
                    "subtitle": format!("{} left{}", clock::clock(timer.left(now)), if paused { ", paused" } else { "" }),
                    "icon": if paused { "play" } else { "pause" },
                    "id": format!("toggle {}", timer.id),
                }));
            }
            // The stopwatch before the presets, so the launcher's
            // max_results doesn't cut it.
            results.extend(self.stopwatch_results(now).into_iter().take(1));
            for preset in &self.settings.presets {
                if let Ok(parsed) = duration::parse(preset) {
                    results.push(start_result(&parsed, preset));
                }
            }
        } else if lower == "sw" || (lower.len() >= 2 && "stopwatch".starts_with(&lower)) {
            results.extend(self.stopwatch_results(now));
            if results.is_empty() {
                results.push(json!({
                    "title": "The stopwatch is off",
                    "subtitle": "It comes with the clock module",
                    "icon": "timer",
                }));
            }
        } else {
            results.push(match duration::parse(query) {
                Ok(parsed) => start_result(&parsed, query),
                Err(error) => json!({
                    "title": "Type a length, like 10m Pizza",
                    "subtitle": error,
                    "icon": "hourglass_top",
                }),
            });
        }
        results
    }

    /// Starting or pausing the clock module's stopwatch, its lap and its
    /// reset; nothing without the clock module.
    fn stopwatch_results(&self, now: u64) -> Vec<Value> {
        if !self.stopwatch.is_object() {
            return Vec::new();
        }
        let since = self.stopwatch["since_ms"].as_u64();
        let banked = self.stopwatch["banked_ms"].as_u64().unwrap_or(0);
        let elapsed = banked + since.map_or(0, |since| now.saturating_sub(since));
        let shown = clock::clock(elapsed);
        let mut results = vec![match since {
            Some(_) => json!({
                "title": "Pause the stopwatch",
                "subtitle": format!("{shown} so far"),
                "icon": "pause",
                "id": "stopwatch toggle",
            }),
            None => json!({
                "title": "Start the stopwatch",
                "subtitle": if elapsed > 0 { format!("On from {shown}") } else { "From 0:00".to_owned() },
                "icon": "timer",
                "id": "stopwatch toggle",
            }),
        }];
        if since.is_some() {
            results.push(json!({
                "title": "Lap",
                "subtitle": "Notes the stopwatch's time",
                "icon": "flag",
                "id": "stopwatch lap",
            }));
        } else if elapsed > 0 {
            results.push(json!({
                "title": "Reset the stopwatch",
                "subtitle": "Back to 0:00, without laps",
                "icon": "restart_alt",
                "id": "stopwatch reset",
            }));
        }
        results
    }

    /// Does what a launcher result offered: `add 10m Pizza`, `toggle 2`, or
    /// `stopwatch toggle` through the clock module.
    fn pick(&mut self, ctx: &ModuleCtx, id: &str, now: u64) -> Result<(), String> {
        let (verb, rest) = id.split_once(' ').unwrap_or((id, ""));
        match verb {
            "add" => {
                self.add(rest, now)?;
            }
            "toggle" => {
                let id = rest
                    .parse()
                    .map_err(|_| format!("no launcher result {id:?}"))?;
                self.timers.toggle(id, now)?;
            }
            "stopwatch" if ["toggle", "lap", "reset"].contains(&rest) => {
                let call = ctx.call("clock", "stopwatch", &[rest]);
                tokio::spawn(async move {
                    if let Err(error) = call.await {
                        tracing::warn!(%error, "the clock's stopwatch");
                    }
                });
                return Ok(());
            }
            _ => return Err(format!("no launcher result {id:?}")),
        }
        self.changed(ctx, false);
        Ok(())
    }
}

/// What a custom timer's bubble and the views get. `count` is how many
/// timers the bubble stands for: all of them for the one bubble with
/// `timer_bubbles = "soonest"`, else 1.
fn timer_payload(timer: &Custom, now: u64, count: usize) -> Value {
    json!({
        "phase": "timer",
        "id": timer.id,
        "label": timer.label,
        "name": timer.name(),
        "paused": timer.paused(),
        "ends_ms": timer.ends_ms,
        "left_ms": timer.left(now),
        "total_ms": timer.total_ms,
        "count": count,
    })
}

/// The launcher's result that starts a timer from what was typed.
fn start_result(parsed: &duration::Parsed, typed: &str) -> Value {
    let length = duration::words(parsed.ms);
    let title = if parsed.label.is_empty() {
        format!("Start a {length} timer")
    } else {
        format!("Start a {length} timer for {}", parsed.label)
    };
    json!({
        "title": title,
        "subtitle": "Counts down in a bubble, with the alarm at the end",
        "icon": "hourglass_top",
        "id": format!("add {typed}"),
    })
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
    fn checks_the_alarm_and_the_presets() {
        let load = |text: &str| Settings::load(&text.parse().unwrap());
        assert!(load("").is_ok());
        assert_eq!(
            load("").unwrap().alarm(),
            Some(Sound::Name(ALARM.to_owned()))
        );
        assert_eq!(
            load("sound_file = \"/tmp/bell.oga\"").unwrap().alarm(),
            Some(Sound::File("/tmp/bell.oga".into()))
        );
        assert!(load("sound_file = \"bell.oga\"").is_err());
        assert!(
            load("presets = [\"5m\", \"Tea\"]")
                .unwrap_err()
                .contains("presets")
        );
        assert!(load("timer_bubbles = \"soonest\"").is_ok());
        assert!(load("timer_bubbles = \"some\"").is_err());
    }

    #[test]
    fn the_timers_survive_a_save() {
        let dir = std::env::temp_dir().join(format!("mochi-timer-{}", std::process::id()));
        let mut saved = Saved::default();
        saved.clock.sessions = 2;
        saved.clock.start(Phase::Break, 5, now());
        saved.timers.add(60_000, "Tea", now()).unwrap();
        save(&dir, &saved);
        assert_eq!(load(&dir), saved);
        // A file from before custom timers loads as the focus timer.
        std::fs::write(dir.join(FILE), r#"{"countdown":null,"sessions":3}"#).unwrap();
        let old = load(&dir);
        assert_eq!((old.clock.sessions, old.timers), (3, Timers::default()));
        std::fs::write(dir.join(FILE), "not json").unwrap();
        assert_eq!(load(&dir), Saved::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn state() -> State {
        let settings = Settings::default();
        State {
            speaker: settings.speaker(),
            settings,
            clock: Clock::default(),
            timers: Timers::default(),
            dir: PathBuf::new(),
            bubble: None,
            bubbles: BTreeMap::new(),
            fresh: None,
            notice: None,
            rang: Vec::new(),
            stopwatch: Value::Null,
        }
    }

    #[test]
    fn the_launcher_offers_timers_and_the_stopwatch() {
        let mut state = state();
        let titles = |results: Vec<Value>| -> Vec<String> {
            results
                .iter()
                .map(|result| result["title"].as_str().unwrap().to_owned())
                .collect()
        };
        let now = 1_790_000_000_000;
        let typed = state.search(" 10m Pizza ", now);
        assert_eq!(titles(typed.clone()), ["Start a 10 min timer for Pizza"]);
        assert_eq!(typed[0]["id"], "add 10m Pizza");
        let wrong = state.search("Pizza", now);
        assert_eq!(wrong[0]["id"], Value::Null);
        assert!(
            wrong[0]["subtitle"]
                .as_str()
                .unwrap()
                .contains("isn't a length")
        );

        // Nothing typed: the presets; the stopwatch only with the clock.
        assert_eq!(state.search("", now).len(), 5);
        assert_eq!(titles(state.search("sw", now)), ["The stopwatch is off"]);
        state.stopwatch = json!({ "since_ms": null, "banked_ms": 0, "laps": [] });
        assert_eq!(titles(state.search("sw", now)), ["Start the stopwatch"]);
        state.stopwatch = json!({ "since_ms": now - 65_000, "banked_ms": 0, "laps": [] });
        let running = state.search("stop", now);
        assert_eq!(titles(running.clone()), ["Pause the stopwatch", "Lap"]);
        assert_eq!(running[0]["subtitle"], "1:05 so far");
        assert_eq!(running[0]["id"], "stopwatch toggle");

        // A running timer comes first, to pause.
        state.add("5m Tea", now).unwrap();
        let empty = state.search("", now + 60_000);
        assert_eq!(empty[0]["title"], "Pause Tea");
        assert_eq!(empty[0]["subtitle"], "4:00 left");
        assert_eq!(empty[0]["id"], "toggle 1");
    }
}

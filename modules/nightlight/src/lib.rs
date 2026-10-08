//! Night light: warmer screens in the evening, by setting each output's
//! gamma through `wlr-gamma-control-unstable-v1`. Hyprland, Sway and niri
//! have it, so nothing else needs to run.
//!
//! It turns on and off by hand, between two times of day, or between
//! sunset and sunrise where you are, with a fade at each end. Turning it on
//! or off by hand while a schedule runs lasts until the schedule changes
//! next. The hub has a tile for it.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.nightlight]
//! temperature = 4000     # kelvin at night, from 1000 to 6000
//! schedule = "manual"    # manual, times or sun
//! start = "20:00"        # with times
//! end = "07:00"
//! latitude = 48.85       # with sun
//! longitude = 2.35
//! fade_minutes = 30
//! ```

mod color;
mod gamma;
mod schedule;
mod tour;

use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ArgSpec, Assets, BoxFuture, ContributionSpec, Module, ModuleCommand, ModuleCtx,
    ModuleError, ModuleEvent,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::color::NEUTRAL;
use crate::schedule::{Now, Sun};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How often the schedule is checked, which is also the fade's step.
const TICK: Duration = Duration::from_secs(30);
/// Turning it on or off by hand slides there in this many steps…
const SLIDE_STEPS: u32 = 12;
/// …this far apart.
const SLIDE_STEP: Duration = Duration::from_millis(60);

#[derive(Debug, Default)]
pub struct Nightlight;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Schedule {
    /// Only by hand.
    #[default]
    Manual,
    /// From `start` to `end`.
    Times,
    /// From sunset to sunrise at `latitude` and `longitude`.
    Sun,
}

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// The temperature at night, in kelvin; 6500 is daylight.
    #[schemars(range(min = 1000, max = 6000))]
    temperature: u32,
    /// When it turns on by itself.
    schedule: Schedule,
    /// When night starts, with `schedule = "times"`.
    start: String,
    /// When night ends, with `schedule = "times"`.
    end: String,
    /// Where you are, with `schedule = "sun"`: north is positive.
    #[schemars(range(min = -90, max = 90))]
    latitude: f64,
    /// East is positive.
    #[schemars(range(min = -180, max = 180))]
    longitude: f64,
    /// How long it takes to warm up or cool down on a schedule.
    #[schemars(range(min = 0, max = 180))]
    fade_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            temperature: 4000,
            schedule: Schedule::Manual,
            start: "20:00".into(),
            end: "07:00".into(),
            latitude: 0.0,
            longitude: 0.0,
            fade_minutes: 30,
        }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if !(color::WARMEST..=6000).contains(&settings.temperature) {
            return Err(format!(
                "temperature is {}; it goes from {} to 6000",
                settings.temperature,
                color::WARMEST
            ));
        }
        schedule::parse_time(&settings.start).map_err(|error| format!("start: {error}"))?;
        schedule::parse_time(&settings.end).map_err(|error| format!("end: {error}"))?;
        if !(-90.0..=90.0).contains(&settings.latitude) {
            return Err(format!(
                "latitude is {}; it goes from -90 to 90",
                settings.latitude
            ));
        }
        if !(-180.0..=180.0).contains(&settings.longitude) {
            return Err(format!(
                "longitude is {}; it goes from -180 to 180",
                settings.longitude
            ));
        }
        Ok(settings)
    }

    /// How far into the night the schedule is now, from 0 to 1. Near the
    /// poles, a day may have no night or be all night.
    fn scheduled(&self, now: Now) -> f64 {
        let fade = f64::from(self.fade_minutes);
        match self.schedule {
            Schedule::Manual => 0.0,
            // load checked both.
            Schedule::Times => schedule::night(
                now.minute,
                schedule::parse_time(&self.start).unwrap_or(1200.0),
                schedule::parse_time(&self.end).unwrap_or(420.0),
                fade,
            ),
            Schedule::Sun => match schedule::sun(self.latitude, self.longitude, now) {
                Sun::Crosses { rise, set } => schedule::night(now.minute, set, rise, fade),
                Sun::Night => 1.0,
                Sun::Day => 0.0,
            },
        }
    }
}

impl Module for Nightlight {
    fn id(&self) -> &'static str {
        "nightlight"
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

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("hub", "card", "toggle", "Card", "Night light")
                .icon("nightlight")
                .order(13)
                .options(json!({ "span": 1, "rows": 1 })),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("on", "Warm the screens until the schedule changes next"),
            ActionSpec::new("off", "Back to daylight until the schedule changes next"),
            ActionSpec::new("toggle", "Turn it on or off"),
            ActionSpec::new("auto", "Follow the schedule again"),
            ActionSpec::new(
                "temperature",
                "Set the night's temperature until mochid restarts",
            )
            .arg(ArgSpec::string("kelvin", "From 1000 to 6000, like 3500")),
            ActionSpec::new("status", "Print whether it's on, and how warm"),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused anything out of range.
            let settings: Settings = ctx.settings()?;
            let (problems, mut reported) = mpsc::unbounded_channel();
            let gamma = match gamma::start(problems) {
                Ok(gamma) => Some(gamma),
                Err(error) => {
                    tracing::warn!(%error, "no night light");
                    None
                }
            };
            let mut state = State {
                kelvin: settings.temperature,
                settings,
                problem: gamma
                    .is_none()
                    .then(|| "the compositor doesn't let Mochi set the gamma".to_owned()),
                gamma,
                forced: None,
                applied: NEUTRAL,
                target: NEUTRAL,
                slide: None,
            };
            state.update(&ctx, false);
            let mut tick = tokio::time::interval(TICK);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut slide = tokio::time::interval(SLIDE_STEP);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(_) => {}
                    },
                    _ = tick.tick() => state.update(&ctx, false),
                    _ = slide.tick(), if state.slide.is_some() => state.step(&ctx),
                    Some(problem) = reported.recv() => {
                        tracing::warn!(%problem, "night light");
                        state.problem = Some(problem);
                        state.publish(&ctx);
                    }
                }
            }
        })
    }
}

#[derive(Debug)]
struct State {
    settings: Settings,
    /// The night's temperature: the setting, or what `temperature` set.
    kelvin: u32,
    gamma: Option<gamma::Gamma>,
    problem: Option<String>,
    /// On or off by hand, and whether the schedule said night then: it
    /// lasts until the schedule says otherwise.
    forced: Option<(bool, bool)>,
    /// The temperature on the screens.
    applied: u32,
    /// Where it's going.
    target: u32,
    /// A slide by hand: where it started and how many steps are done.
    slide: Option<(u32, u32)>,
}

impl State {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let scheduled = self.settings.scheduled(Now::local()) > 0.0;
        let result = match command.action.as_str() {
            "on" => {
                self.forced = Some((true, scheduled));
                Ok(())
            }
            "off" => {
                self.forced = Some((false, scheduled));
                Ok(())
            }
            "toggle" => {
                self.forced = Some((!self.on(), scheduled));
                Ok(())
            }
            "auto" => {
                self.forced = None;
                Ok(())
            }
            "temperature" => command
                .args
                .str("kelvin")
                .unwrap_or_default()
                .trim_end_matches(['K', 'k'])
                .parse::<u32>()
                .ok()
                .filter(|kelvin| (color::WARMEST..=6000).contains(kelvin))
                .map(|kelvin| self.kelvin = kelvin)
                .ok_or_else(|| "give a temperature from 1000 to 6000, like 3500".to_owned()),
            "status" => return command.answer(Ok(self.status())),
            other => Err(format!("nightlight has no action {other}")),
        };
        if result.is_ok() {
            // Try again: the compositor says if it still refuses.
            if self.gamma.is_some() {
                self.problem = None;
            }
            self.update(ctx, true);
        }
        command.reply(result);
    }

    /// Whether it's on now: by hand, or by the schedule.
    fn on(&self) -> bool {
        self.amount(Now::local()) > 0.0
    }

    /// How warm it should be, from 0 for daylight to 1 for the night's
    /// temperature.
    fn amount(&self, now: Now) -> f64 {
        match self.forced {
            Some((on, _)) => {
                if on {
                    1.0
                } else {
                    0.0
                }
            }
            None => self.settings.scheduled(now),
        }
    }

    /// Follows the schedule; `by_hand` slides there instead of jumping.
    fn update(&mut self, ctx: &ModuleCtx, by_hand: bool) {
        let now = Now::local();
        let scheduled = self.settings.scheduled(now) > 0.0;
        if let Some((_, then)) = self.forced
            && then != scheduled
        {
            // The schedule moved on: it's in charge again.
            self.forced = None;
        }
        let target = schedule::blend(NEUTRAL, self.kelvin, self.amount(now));
        if target != self.target {
            self.target = target;
            if by_hand {
                self.slide = Some((self.applied, 0));
            } else {
                self.slide = None;
                self.apply(target);
            }
        }
        self.publish(ctx);
    }

    /// One step of a slide by hand.
    fn step(&mut self, ctx: &ModuleCtx) {
        let Some((from, done)) = self.slide else {
            return;
        };
        let done = done + 1;
        let kelvin = schedule::blend(from, self.target, f64::from(done) / f64::from(SLIDE_STEPS));
        self.apply(kelvin);
        if done >= SLIDE_STEPS {
            self.slide = None;
            self.publish(ctx);
        } else {
            self.slide = Some((from, done));
        }
    }

    fn apply(&mut self, kelvin: u32) {
        self.applied = kelvin;
        if let Some(gamma) = &self.gamma
            && let Err(error) = gamma.set(Some(kelvin))
        {
            self.problem = Some(error);
        }
    }

    fn payload(&self) -> Value {
        json!({
            "on": self.target < NEUTRAL,
            "temperature": self.kelvin,
            "current": self.target,
            "schedule": match self.settings.schedule {
                Schedule::Manual => "manual",
                Schedule::Times => "times",
                Schedule::Sun => "sun",
            },
            "forced": self.forced.is_some(),
            "available": self.gamma.is_some(),
            "problem": self.problem,
        })
    }

    fn publish(&self, ctx: &ModuleCtx) {
        ctx.publish_state(self.payload());
    }

    fn status(&self) -> String {
        let mut status = if self.target < NEUTRAL {
            format!("on, {} K", self.target)
        } else {
            "off".to_owned()
        };
        if self.forced.is_some() && self.settings.schedule != Schedule::Manual {
            status.push_str(", by hand until the schedule changes");
        }
        if let Some(problem) = &self.problem {
            status.push_str(&format!("\n{problem}"));
        }
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> mochi_core::toml::Table {
        mochi_core::toml::from_str(text).unwrap()
    }

    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<Settings>(
            "nightlight",
            include_str!("../settings.toml"),
        );
    }

    #[test]
    fn settings_stay_in_range() {
        assert!(Settings::load(&table("temperature = 3000")).is_ok());
        assert!(Settings::load(&table("temperature = 6500")).is_err());
        assert!(Settings::load(&table("start = \"25:00\"")).is_err());
        assert!(Settings::load(&table("latitude = 91.0")).is_err());
        assert!(Settings::load(&table("schedule = \"weekly\"")).is_err());
    }

    #[test]
    fn the_schedule_says_how_far_into_the_night() {
        let settings = Settings::load(&table("schedule = \"times\"\nfade_minutes = 0")).unwrap();
        let at = |minute: f64| Now {
            minute,
            day: 1,
            offset: 0.0,
        };
        assert_eq!(settings.scheduled(at(1260.0)), 1.0);
        assert_eq!(settings.scheduled(at(720.0)), 0.0);
        let manual = Settings::default();
        assert_eq!(manual.scheduled(at(1260.0)), 0.0);
        // Near the pole in June, the sun never sets.
        let north = Settings::load(&table(
            "schedule = \"sun\"\nlatitude = 69.65\nlongitude = 18.96",
        ))
        .unwrap();
        let june = Now {
            minute: 0.0,
            day: 172,
            offset: 0.0,
        };
        assert_eq!(north.scheduled(june), 0.0);
        assert_eq!(north.scheduled(Now { day: 355, ..june }), 1.0);
    }
}

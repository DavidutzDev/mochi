//! The volume mixer: the output and input with their volumes and a choice
//! of device, and a volume and mute for each app playing sound, from the
//! audio server over the PulseAudio protocol.
//!
//! It shows as a hub page, and `mochi ipc audio toggle`, bound to a key,
//! opens the same mixer on the island. The `volume`, `mute`, `output`,
//! `input` and `move` actions change things from keybinds and scripts.
//!
//! The streams of one app share a row. Each slider shows a peak meter while
//! a view of the mixer is open: the view sends `meters on` every few
//! seconds and `meters off` when it closes, and only then does the audio
//! thread open its peak-detecting streams. The levels go to the views as
//! live values, 20 times a second, so they never touch the module's state.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.audio]
//! max_volume = 100   # the top of the sliders and the OSD's bar, up to 300
//! ```

mod meter;
mod mixer;
mod pulse;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, ContributionSpec,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::meter::Peaks;
use crate::pulse::{Command, Handle, Report, Snapshot, Target};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The loudest a slider goes; past it, sound distorts.
const LOUDEST: u32 = 300;
/// How often the meters' levels go to the views.
const METER_TICK: Duration = Duration::from_millis(50);
/// How long a view's `meters on` lasts; views send it again before then, so
/// meters stop on their own when the UI goes away without saying so.
const METER_LEASE: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct Audio;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    max_volume: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { max_volume: 100 }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if !(1..=LOUDEST).contains(&settings.max_volume) {
            return Err(format!(
                "max_volume is {}; it goes from 1 to {LOUDEST}",
                settings.max_volume
            ));
        }
        Ok(settings)
    }
}

impl Module for Audio {
    fn id(&self) -> &'static str {
        "audio"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        Settings::load(table).map(drop)
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("hub", "page", "mixer", "Page", "Sound")
                .icon("volume")
                .order(20),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let target = || {
            ArgSpec::string(
                "target",
                "output, input, a device's name, an app's name for all its streams, or a stream's id",
            )
        };
        vec![
            ActionSpec::new("toggle", "Open the mixer on the island, or close it"),
            ActionSpec::new("open", "Open the mixer on the island"),
            ActionSpec::new("close", "Close the mixer"),
            ActionSpec::new("volume", "Set a volume")
                .arg(target())
                .arg(ArgSpec::string(
                    "level",
                    "A percent like 40, or +5 and -5 to move it",
                )),
            ActionSpec::new("mute", "Mute or unmute").arg(target()).arg(
                ArgSpec::choice("state", "Mute, unmute, or flip it", ["on", "off", "toggle"])
                    .optional(),
            ),
            ActionSpec::new("output", "Play sound through another output").arg(ArgSpec::string(
                "name",
                "The device's name, as the mixer lists it",
            )),
            ActionSpec::new("input", "Record from another input").arg(ArgSpec::string(
                "name",
                "The device's name, as the mixer lists it",
            )),
            ActionSpec::new("move", "Play an app through another output")
                .arg(ArgSpec::string(
                    "app",
                    "An app's name for all its streams, or a stream's id",
                ))
                .arg(ArgSpec::string(
                    "device",
                    "The output's name or description, or output for the one in use",
                )),
            ActionSpec::new(
                "meters",
                "Run the level meters for a view of the mixer; the mixer sends this",
            )
            .arg(ArgSpec::string("view", "A name the view picks for itself"))
            .arg(ArgSpec::choice(
                "state",
                "on for the next 10 seconds, or off",
                ["on", "off"],
            )),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings already refused anything out of range.
            let settings: Settings = ctx.settings()?;
            let (sender, mut reports) = mpsc::unbounded_channel();
            let peaks = Peaks::default();
            let handle = pulse::spawn(sender, peaks.clone())?;
            let mut state = State {
                max_volume: settings.max_volume.clamp(1, LOUDEST),
                handle,
                snapshot: None,
                shown: None,
                peaks,
                viewers: HashMap::new(),
                levels: HashMap::new(),
                sent: Value::Null,
            };
            state.publish(&ctx);
            let mut ticks = tokio::time::interval(METER_TICK);
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::Ended { activity, .. }) if state.shown == Some(activity) => {
                            state.shown = None;
                        }
                        Some(_) => {}
                    },
                    Some(report) = reports.recv() => {
                        state.snapshot = match report {
                            Report::Snapshot(snapshot) => Some(snapshot),
                            Report::Lost => None,
                        };
                        state.publish(&ctx);
                    }
                    _ = ticks.tick(), if !state.viewers.is_empty() => state.meter(&ctx),
                }
            }
        })
    }
}

#[derive(Debug)]
struct State {
    max_volume: u32,
    handle: Handle,
    /// `None` while the audio server isn't connected.
    snapshot: Option<Snapshot>,
    shown: Option<ActivityId>,
    /// What the audio thread's meters measured since the last tick.
    peaks: Peaks,
    /// The views that want meters, and until when.
    viewers: HashMap<String, Instant>,
    /// Each meter's level as last sent, to let it fall.
    levels: HashMap<String, f32>,
    /// The levels last sent, to send nothing while they stay the same.
    sent: Value,
}

impl State {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let result = match command.action.as_str() {
            "toggle" if self.shown.is_some() => {
                self.close(ctx);
                Ok(())
            }
            "toggle" | "open" => {
                self.open(ctx);
                Ok(())
            }
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "volume" => self.targets(&command).and_then(|(targets, (now, _))| {
                let level = command.args.str("level").unwrap_or_default();
                let level = mixer::level(level, now, self.max_volume.max(now))?;
                targets
                    .into_iter()
                    .try_for_each(|target| self.handle.send(Command::Volume(target, level)))
            }),
            "mute" => self.targets(&command).and_then(|(targets, (_, muted))| {
                let muted = match command.args.str("state") {
                    Some("on") => true,
                    Some("off") => false,
                    _ => !muted,
                };
                targets
                    .into_iter()
                    .try_for_each(|target| self.handle.send(Command::Mute(target, muted)))
            }),
            "move" => self.move_app(&command),
            "meters" => {
                let view = command.args.str("view").unwrap_or_default().to_owned();
                if command.args.str("state") == Some("off") {
                    self.viewers.remove(&view);
                } else {
                    self.viewers.insert(view, Instant::now() + METER_LEASE);
                }
                self.follow_viewers()
            }
            "output" | "input" => {
                let name = command.args.str("name").unwrap_or_default().to_owned();
                let snapshot = self.snapshot.as_ref();
                let output = command.action == "output";
                let known = snapshot.is_some_and(|snapshot| {
                    let devices = if output {
                        &snapshot.sinks
                    } else {
                        &snapshot.sources
                    };
                    devices.iter().any(|device| device.name == name)
                });
                if known {
                    self.handle.send(if output {
                        Command::DefaultSink(name)
                    } else {
                        Command::DefaultSource(name)
                    })
                } else {
                    Err(format!("no {} called {name}", command.action))
                }
            }
            other => Err(format!("audio has no action {other}")),
        };
        command.reply(result);
    }

    /// What a command names, with its volume and mute now.
    fn targets(&self, command: &ModuleCommand) -> Result<(Vec<Target>, (u32, bool)), String> {
        let snapshot = self.connected()?;
        let targets = mixer::targets(snapshot, command.args.str("target").unwrap_or_default())?;
        let current = mixer::current(snapshot, &targets).ok_or("it just went away")?;
        Ok((targets, current))
    }

    fn connected(&self) -> Result<&Snapshot, String> {
        self.snapshot
            .as_ref()
            .ok_or_else(|| "the audio server isn't connected".to_owned())
    }

    /// Moves every stream of an app, or one stream, to another output.
    /// PipeWire remembers it for the app the next time it plays.
    fn move_app(&self, command: &ModuleCommand) -> Result<(), String> {
        let snapshot = self.connected()?;
        let streams = mixer::streams(snapshot, command.args.str("app").unwrap_or_default())?;
        let sink = mixer::sink(snapshot, command.args.str("device").unwrap_or_default())?;
        streams
            .into_iter()
            .try_for_each(|stream| self.handle.send(Command::Move(stream, sink.to_owned())))
    }

    /// Drops the views that stopped asking, and tells the audio thread
    /// whether the meters should run.
    fn follow_viewers(&mut self) -> Result<(), String> {
        let now = Instant::now();
        self.viewers.retain(|_, until| *until > now);
        let metering = !self.viewers.is_empty();
        if !metering {
            self.levels.clear();
            self.sent = Value::Null;
        }
        self.handle.send(Command::Meters(metering))
    }

    /// Sends the views the meters' levels, when they changed.
    fn meter(&mut self, ctx: &ModuleCtx) {
        let now = Instant::now();
        if self.viewers.values().any(|until| *until <= now) {
            let _ = self.follow_viewers();
            if self.viewers.is_empty() {
                return;
            }
        }
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let peaks = meter::take(&self.peaks);
        for (key, level) in &mut self.levels {
            if !peaks.contains_key(key) {
                *level = mixer::fall(*level, None);
            }
        }
        for (key, peak) in peaks {
            let level = self.levels.entry(key).or_default();
            *level = mixer::fall(*level, Some(peak));
        }
        self.levels.retain(|_, level| *level > 0.0);
        let levels = mixer::levels(snapshot, &self.levels);
        if levels != self.sent {
            ctx.publish_live(levels.clone());
            self.sent = levels;
        }
    }

    fn open(&mut self, ctx: &ModuleCtx) {
        // The other panels take the keyboard too; only one can be open. Not
        // awaited, as they close this the same way.
        for module in ["hub", "launcher", "clipboard", "tray", "emoji"] {
            let close = ctx.call(module, "close", &[]);
            tokio::spawn(async move {
                match close.await {
                    Ok(()) | Err(CallError::NotEnabled(_)) => {}
                    Err(error) => tracing::warn!(%error, module, "could not close it"),
                }
            });
        }
        let spec = ActivitySpec::new("Panel")
            .key("audio")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(self.payload());
        self.shown = Some(ctx.present(spec));
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown.take() {
            ctx.withdraw(id);
        }
    }

    fn payload(&self) -> Value {
        mixer::payload(self.snapshot.as_ref(), self.max_volume)
    }

    /// Tells the hub page and the island what changed.
    fn publish(&self, ctx: &ModuleCtx) {
        let payload = self.payload();
        if let Some(id) = self.shown {
            ctx.update(id, payload.clone());
        }
        ctx.publish_state(payload);
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "audio",
            include_str!("../settings.toml"),
        );
    }

    #[test]
    fn max_volume_stays_in_range() {
        let table = |max: u32| mochi_core::toml::from_str(&format!("max_volume = {max}")).unwrap();
        assert!(super::Settings::load(&table(200)).is_ok());
        assert!(super::Settings::load(&table(300)).is_ok());
        assert!(super::Settings::load(&table(0)).is_err());
        assert!(super::Settings::load(&table(301)).is_err());
    }
}

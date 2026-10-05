//! The volume mixer: the output and input with their volumes and a choice
//! of device, and a volume and mute for each app playing sound, from the
//! audio server over the PulseAudio protocol.
//!
//! It shows as a hub page, and `mochi ipc audio toggle`, bound to a key,
//! opens the same mixer on the island. The `volume`, `mute`, `output` and
//! `input` actions change things from keybinds and scripts.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.audio]
//! max_volume = 100   # the top of the sliders and the OSD's bar, up to 300
//! ```

mod mixer;
mod pulse;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, ContributionSpec,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::pulse::{Command, Handle, Report, Snapshot, Target};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The loudest a slider goes; past it, sound distorts.
const LOUDEST: u32 = 300;

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
                "output, input, an app's id from the mixer, or a device's name",
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
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings already refused anything out of range.
            let settings: Settings = ctx.settings()?;
            let (sender, mut reports) = mpsc::unbounded_channel();
            let handle = pulse::spawn(sender)?;
            let mut state = State {
                max_volume: settings.max_volume.clamp(1, LOUDEST),
                handle,
                snapshot: None,
                shown: None,
            };
            state.publish(&ctx);

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
            "volume" => self.target(&command).and_then(|(target, (now, _))| {
                let level = command.args.str("level").unwrap_or_default();
                let level = mixer::level(level, now, self.max_volume.max(now))?;
                self.handle.send(Command::Volume(target, level))
            }),
            "mute" => self.target(&command).and_then(|(target, (_, muted))| {
                let muted = match command.args.str("state") {
                    Some("on") => true,
                    Some("off") => false,
                    _ => !muted,
                };
                self.handle.send(Command::Mute(target, muted))
            }),
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
    fn target(&self, command: &ModuleCommand) -> Result<(Target, (u32, bool)), String> {
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or("the audio server isn't connected")?;
        let target = mixer::target(snapshot, command.args.str("target").unwrap_or_default())?;
        let current = mixer::current(snapshot, &target).ok_or("it just went away")?;
        Ok((target, current))
    }

    fn open(&mut self, ctx: &ModuleCtx) {
        // The other panels take the keyboard too; only one can be open. Not
        // awaited, as they close this the same way.
        for module in ["hub", "launcher", "clipboard", "tray"] {
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

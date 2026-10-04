//! On-screen display for changes made anywhere on the system: output volume
//! and mute, the default output device, microphone mute, Caps Lock and Num
//! Lock. It only listens; nothing needs to call it.
//!
//! All notices share one slot: the newest replaces whatever OSD is shown.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.osd]
//! timeout_ms = 1500
//! volume = true       # output volume and mute
//! device = true       # default output device switches
//! microphone = true   # microphone mute
//! locks = true        # Caps Lock and Num Lock
//! ```

mod audio;
mod locks;
mod notice;

use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActivitySpec, Assets, BoxFuture, Module, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::notice::{Notice, Tracker};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// Every OSD uses this key, so a new one replaces the one on screen.
const KEY: &str = "osd";

#[derive(Debug, Default)]
pub struct Osd;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    timeout_ms: u64,
    volume: bool,
    device: bool,
    microphone: bool,
    locks: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timeout_ms: 1500,
            volume: true,
            device: true,
            microphone: true,
            locks: true,
        }
    }
}

impl Settings {
    fn shows(&self, notice: &Notice) -> bool {
        match notice {
            Notice::Volume { .. } => self.volume,
            Notice::Device { .. } => self.device,
            Notice::Microphone { .. } => self.microphone,
            Notice::Lock { .. } => self.locks,
        }
    }

    fn wants_audio(&self) -> bool {
        self.volume || self.device || self.microphone
    }
}

impl Module for Osd {
    fn id(&self) -> &'static str {
        "osd"
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

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let timeout = Duration::from_millis(settings.timeout_ms);

            let (sender, mut changes) = mpsc::unbounded_channel();
            if settings.wants_audio() {
                audio::spawn(sender.clone())?;
            }
            if settings.locks {
                tokio::spawn(locks::watch(sender.clone()));
            }
            drop(sender);

            let mut tracker = Tracker::default();
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            command.reply(Err("osd has no actions".into()));
                        }
                        Some(_) => {}
                    },
                    Some(change) = changes.recv() => {
                        if let Some(notice) = tracker.apply(change)
                            && settings.shows(&notice)
                        {
                            tracing::debug!(?notice, "osd");
                            ctx.present(spec(&notice, timeout));
                        }
                    }
                }
            }
        })
    }
}

fn spec(notice: &Notice, timeout: Duration) -> ActivitySpec {
    ActivitySpec::new(notice.view())
        .key(KEY)
        .priority(Priority::HIGH)
        .timeout(timeout)
        .payload(notice.payload())
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "osd",
            include_str!("../settings.toml"),
        );
    }
}

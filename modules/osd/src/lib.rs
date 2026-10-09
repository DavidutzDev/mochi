//! On-screen display for changes made anywhere on the system: output volume
//! and mute, the default output device, microphone mute, Caps Lock, Num
//! Lock and the keyboard layout. It only listens; nothing needs to call it.
//!
//! All notices share one slot: the newest replaces whatever OSD is shown.
//!
//! It also publishes the keyboard layout for any view: `keyboard_layout` in
//! its state, the layout's name, or null when the compositor doesn't say.
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
//! layout = true       # keyboard layout switches
//! ```

mod audio;
mod layout;
mod locks;
mod notice;
mod tour;

use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActivitySpec, Assets, BoxFuture, ContributionSpec, Module, ModuleCtx, ModuleError, ModuleEvent,
    Priority, SamePriority,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

use crate::notice::{Change, Notice, Tracker};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// Every OSD uses this key, so a new one replaces the one on screen.
const KEY: &str = "osd";

#[derive(Debug, Default)]
pub struct Osd;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    timeout_ms: u64,
    volume: bool,
    device: bool,
    microphone: bool,
    locks: bool,
    layout: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timeout_ms: 1500,
            volume: true,
            device: true,
            microphone: true,
            locks: true,
            layout: true,
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
            Notice::Layout { .. } => self.layout,
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
            // Watched even with its notice off, for the published state.
            tokio::spawn(layout::watch(ctx.compositor().clone(), sender.clone()));
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
                        if let Change::Layout(layout) = &change {
                            ctx.publish_state(json!({ "keyboard_layout": layout }));
                        }
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
        // Over the workspace notice, which then ends: the newest feedback
        // wins.
        .same_priority(SamePriority::Stack)
        // Feedback for a key you just pressed: clicks go on to your windows,
        // and it never shows late, after a panel closes.
        .passive()
        .fleeting()
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

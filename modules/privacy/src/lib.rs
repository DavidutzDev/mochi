//! Privacy: a bubble next to the island while an app records from a
//! microphone or has a camera open: a microphone icon in orange, a camera
//! in green. A click on it mutes the microphone or unmutes it, and the
//! wide pill names the apps.
//!
//! The apps recording come from the audio module's state, which leaves out
//! Mochi's own meters and recordings of what an output plays. The camera
//! comes from `/proc`: the processes with a `/dev/video*` device open.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.privacy]
//! microphone = true   # apps recording, from the audio module
//! camera = true       # apps with a camera open
//! ```

mod camera;
mod tour;

use std::path::Path;
use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, Area, Assets, BoxFuture, BubbleId, BubbleSpec, ContributionSpec, Module, ModuleCtx,
    ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How often `/proc` is read for cameras.
const CAMERA_POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub struct Privacy;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// Apps recording from a microphone, which the audio module reports.
    microphone: bool,
    /// Apps with a camera open, checked every 2 seconds.
    camera: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            microphone: true,
            camera: true,
        }
    }
}

impl Module for Privacy {
    fn id(&self) -> &'static str {
        "privacy"
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
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        tour::steps()
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![ActionSpec::new(
            "status",
            "Print the apps using the microphone and the camera",
        )]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            if settings.microphone {
                ctx.watch_state("audio");
            }
            let mut users = Users::default();
            let mut bubble: Option<BubbleId> = None;
            let mut poll = tokio::time::interval(CAMERA_POLL);
            poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                let before = users.clone();
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            if command.action == "status" {
                                command.answer(Ok(users.status()));
                            } else {
                                let error = format!("privacy has no action {}", command.action);
                                command.reply(Err(error));
                            }
                        }
                        Some(ModuleEvent::State { module, state }) if module == "audio" => {
                            users.follow_audio(&state);
                        }
                        Some(ModuleEvent::BubbleClicked(_)) => {
                            let mute = ctx.call("audio", "mute", &["input"]);
                            tokio::spawn(async move {
                                if let Err(error) = mute.await {
                                    tracing::debug!(%error, "can't mute the microphone");
                                }
                            });
                        }
                        Some(_) => {}
                    },
                    _ = poll.tick(), if settings.camera => {
                        let own = std::process::id();
                        let found = tokio::task::spawn_blocking(move || {
                            camera::users(Path::new("/proc"), own)
                        })
                        .await
                        .unwrap_or_default();
                        users.camera = found;
                    }
                }
                if users != before {
                    tracing::debug!(?users, "privacy");
                    show(&ctx, &mut bubble, &users, &before);
                }
            }
        })
    }
}

/// Who uses what right now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Users {
    microphone: Vec<String>,
    camera: Vec<String>,
    /// The default microphone is muted, so whoever records hears nothing.
    muted: bool,
}

impl Users {
    fn follow_audio(&mut self, state: &Value) {
        self.microphone = state["recording"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|app| app["name"].as_str().map(str::to_owned))
            .collect();
        self.muted = state["input"]["muted"] == true;
    }

    fn active(&self) -> bool {
        !self.microphone.is_empty() || !self.camera.is_empty()
    }

    fn payload(&self) -> Value {
        json!({
            "microphone": self.microphone,
            "camera": self.camera,
            "muted": self.muted,
        })
    }

    fn status(&self) -> String {
        let list = |apps: &[String]| {
            if apps.is_empty() {
                "nobody".to_owned()
            } else {
                apps.join(", ")
            }
        };
        let muted = if self.muted { " (muted)" } else { "" };
        format!(
            "microphone{muted}: {}\ncamera: {}",
            list(&self.microphone),
            list(&self.camera)
        )
    }
}

fn show(ctx: &ModuleCtx, bubble: &mut Option<BubbleId>, users: &Users, before: &Users) {
    if !users.active() {
        if let Some(id) = bubble.take() {
            ctx.hide_bubble(id);
        }
        return;
    }
    let spec = BubbleSpec::new("Dot")
        .key("dot")
        .wide("Wide")
        .area(Area::CenterRight)
        // Beside the recording and sharing bubbles, which come first.
        .order(-8)
        .priority(Priority::HIGH)
        .payload(users.payload());
    // A new app listening is news; one leaving or a mute isn't.
    let joined = users
        .microphone
        .iter()
        .chain(&users.camera)
        .any(|app| !before.microphone.contains(app) && !before.camera.contains(app));
    let spec = if bubble.is_some() && joined {
        spec.news()
    } else {
        spec
    };
    *bubble = Some(ctx.show_bubble(spec));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<Settings>("privacy", include_str!("../settings.toml"));
    }

    #[test]
    fn the_microphone_comes_from_the_audio_state() {
        let mut users = Users::default();
        users.follow_audio(&json!({
            "recording": [{ "name": "discord", "icon": null }, { "name": "OBS" }],
            "input": { "name": "mic", "muted": true },
        }));
        assert_eq!(users.microphone, ["discord", "OBS"]);
        assert!(users.muted && users.active());
        assert_eq!(
            users.status(),
            "microphone (muted): discord, OBS\ncamera: nobody"
        );
        // The audio module stopped.
        users.follow_audio(&Value::Null);
        assert!(!users.active());
    }
}

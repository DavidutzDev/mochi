//! The idle island: a small clock that shows whenever nothing else does.
//!
//! Settings in `config.toml`:
//!
//! ```toml
//! [module.idle]
//! format = "HH:mm"   # Qt time format
//! ```

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActivitySpec, Assets, BoxFuture, Module, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::json;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Idle;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// Qt time format, as used by `Qt.formatTime`.
    format: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: "HH:mm".into(),
        }
    }
}

impl Module for Idle {
    fn id(&self) -> &'static str {
        "idle"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let pill = || {
                ActivitySpec::new("Pill")
                    .priority(Priority::IDLE)
                    .payload(json!({ "format": settings.format }))
            };

            ctx.present(pill());
            while let Some(event) = ctx.next_event().await {
                match event {
                    // The pill should always be there to fall back to, so a
                    // dismissed one comes straight back.
                    ModuleEvent::Ended { .. } => {
                        ctx.present(pill());
                    }
                    // The daemon only forwards actions the module declares,
                    // and idle declares none.
                    ModuleEvent::Command(command) => {
                        command.reply(Err("idle has no actions".into()))
                    }
                    ModuleEvent::Clicked(_) | ModuleEvent::BubbleClicked(_) => {}
                }
            }
            Ok(())
        })
    }
}

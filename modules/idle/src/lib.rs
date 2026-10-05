//! The idle island: a small clock that shows whenever nothing else does.
//! Clicking it runs another module's action, the hub by default; nothing
//! happens when that module isn't enabled.
//!
//! Settings in `config.toml`:
//!
//! ```toml
//! [module.idle]
//! format = "HH:mm"            # Qt time format
//! click = ["hub", "toggle"]   # module, action, then its arguments; [] for nothing
//! ```

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActivitySpec, Assets, BoxFuture, CallError, Module, ModuleCtx, ModuleError, ModuleEvent,
    Priority,
};
use serde::Deserialize;
use serde_json::json;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Idle;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// Qt time format, as used by `Qt.formatTime`.
    format: String,
    /// What a click runs: a module, its action, then the arguments.
    click: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: "HH:mm".into(),
            click: vec!["hub".into(), "toggle".into()],
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
                    ModuleEvent::Clicked(_) => {
                        if let [module, action, args @ ..] = settings.click.as_slice() {
                            let args: Vec<&str> = args.iter().map(String::as_str).collect();
                            let call = ctx.call(module, action, &args);
                            tokio::spawn(async move {
                                match call.await {
                                    Ok(()) | Err(CallError::NotEnabled(_)) => {}
                                    Err(error) => tracing::warn!(%error, "the click action failed"),
                                }
                            });
                        }
                    }
                    ModuleEvent::BubbleClicked(_) | ModuleEvent::State { .. } => {}
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "idle",
            include_str!("../settings.toml"),
        );
    }
}

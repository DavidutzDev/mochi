//! The idle island: a small clock that shows whenever nothing else does.
//! Clicking it runs another module's action, the hub by default, and
//! resting the pointer on it can run another; nothing happens when that
//! module isn't enabled.
//!
//! Settings in `config.toml`:
//!
//! ```toml
//! [module.idle]
//! format = "HH:mm"                 # Qt time format
//! click = ["hub", "toggle"]        # module, action, then its arguments; [] for nothing
//! hover = ["workspaces", "show"]   # the same, after the pointer rests on it; [] by default
//! hover_delay_ms = 350
//! ```

use std::time::Duration;

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
    /// What resting the pointer on it runs, the same way.
    hover: Vec<String>,
    /// How long the pointer rests before `hover` runs, so passing over it
    /// or clicking it doesn't.
    hover_delay_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: "HH:mm".into(),
            click: vec!["hub".into(), "toggle".into()],
            hover: Vec::new(),
            hover_delay_ms: 350,
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

            let mut shown = ctx.present(pill());
            // When the pointer rests on the pill long enough for `hover`.
            let mut resting: Option<tokio::time::Instant> = None;
            loop {
                let event = tokio::select! {
                    event = ctx.next_event() => match event {
                        Some(event) => event,
                        None => break,
                    },
                    () = wait(resting) => {
                        resting = None;
                        run(&ctx, &settings.hover, "hover");
                        continue;
                    }
                };
                match event {
                    // The pill should always be there to fall back to, so a
                    // dismissed one comes straight back. After a restart the
                    // last run's pill ends too: that one stays gone.
                    ModuleEvent::Ended { activity, .. } if activity == shown => {
                        resting = None;
                        shown = ctx.present(pill());
                    }
                    ModuleEvent::Ended { .. } => {}
                    ModuleEvent::Hovered { activity, hovered } if activity == shown => {
                        resting = (hovered && !settings.hover.is_empty()).then(|| {
                            tokio::time::Instant::now()
                                + Duration::from_millis(settings.hover_delay_ms)
                        });
                    }
                    // The daemon only forwards actions the module declares,
                    // and idle declares none.
                    ModuleEvent::Command(command) => {
                        command.reply(Err("idle has no actions".into()))
                    }
                    // A click before the pointer rested long enough is a
                    // click, not a hover.
                    ModuleEvent::Clicked(_) => {
                        resting = None;
                        run(&ctx, &settings.click, "click");
                    }
                    ModuleEvent::Hovered { .. }
                    | ModuleEvent::BubbleClicked(_)
                    | ModuleEvent::State { .. }
                    | ModuleEvent::Offers(_) => {}
                }
            }
            Ok(())
        })
    }
}

/// Runs `module action args...` from the settings, if any.
fn run(ctx: &ModuleCtx, call: &[String], what: &'static str) {
    if let [module, action, args @ ..] = call {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let call = ctx.call(module, action, &args);
        tokio::spawn(async move {
            match call.await {
                Ok(()) | Err(CallError::NotEnabled(_)) => {}
                Err(error) => tracing::warn!(%error, "the {what} action failed"),
            }
        });
    }
}

/// Sleeps until `deadline`, or forever without one.
async fn wait(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
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

//! The hub: a wide panel that grows out of the island. Its home shows cards
//! and a navbar at the bottom switches to pages. Other modules provide both
//! through [`ContributionSpec`]s with `target = "hub"`:
//!
//! - `card`: a tile on the home screen. `options.span` sets its width in
//!   columns, 1 to 3. Its heading, and a click beside its controls, open
//!   its module's page: `options.page` names one when it has several.
//! - `page`: a tab in the navbar, filling the panel when picked.
//!
//! Contributed views get their module's published state as `payload`, and
//! fill the space the hub gives them. The hub knows nothing about them: a
//! module that isn't enabled simply contributes nothing.
//!
//! `mochi ipc hub toggle`, bound to a key, opens and closes it. Every page
//! gets the same size, `width` and `height` in the settings, so the panel
//! doesn't jump when switching; a page that doesn't fit scrolls.

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, ContributionSpec, Module,
    ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::json;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Hub;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// The panel's width, in logical pixels.
    width: u32,
    /// The room for the cards or a page, above the navbar.
    height: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 860,
            height: 480,
        }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if !(480..=2400).contains(&settings.width) {
            return Err(format!(
                "width is {}; it goes from 480 to 2400",
                settings.width
            ));
        }
        if !(240..=1600).contains(&settings.height) {
            return Err(format!(
                "height is {}; it goes from 240 to 1600",
                settings.height
            ));
        }
        Ok(settings)
    }
}

impl Module for Hub {
    fn id(&self) -> &'static str {
        "hub"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<serde_json::Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        Settings::load(table).map(drop)
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("toggle", "Open the hub, or close it when open"),
            ActionSpec::new("open", "Open the hub").arg(
                ArgSpec::string("page", "A page as module/id, like notifications/history")
                    .optional(),
            ),
            ActionSpec::new("close", "Close the hub"),
        ]
    }

    // The hub's own card goes through the same door as everyone else's.
    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("hub", "card", "clock", "Clock", "Today")
                .icon("clock")
                .order(3)
                .options(json!({ "span": 1, "rows": 1 })),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused sizes out of range.
            let settings: Settings = ctx.settings()?;
            let mut shown: Option<ActivityId> = None;
            while let Some(event) = ctx.next_event().await {
                match event {
                    ModuleEvent::Command(incoming) => command(&ctx, settings, &mut shown, incoming),
                    ModuleEvent::Ended { activity, .. } if shown == Some(activity) => {
                        shown = None;
                    }
                    _ => {}
                }
            }
            Ok(())
        })
    }
}

fn command(
    ctx: &ModuleCtx,
    settings: Settings,
    shown: &mut Option<ActivityId>,
    command: ModuleCommand,
) {
    let result = match command.action.as_str() {
        "toggle" if shown.is_some() => {
            close(ctx, shown);
            Ok(())
        }
        "toggle" | "open" => {
            open(
                ctx,
                settings,
                shown,
                command.args.str("page").unwrap_or("home"),
            );
            Ok(())
        }
        "close" => {
            close(ctx, shown);
            Ok(())
        }
        other => Err(format!("hub has no action {other}")),
    };
    command.reply(result);
}

fn open(ctx: &ModuleCtx, settings: Settings, shown: &mut Option<ActivityId>, page: &str) {
    ctx.close_other_panels();

    let spec = ActivitySpec::new("Hub")
        .key("hub")
        .priority(Priority::URGENT)
        .uninterruptible()
        .modal()
        .payload(json!({
            // Only the island on this monitor takes the keyboard.
            "output": ctx.compositor().state().focused_output,
            "page": page,
            "width": settings.width,
            "height": settings.height,
        }));
    *shown = Some(ctx.present(spec));
}

fn close(ctx: &ModuleCtx, shown: &mut Option<ActivityId>) {
    if let Some(id) = shown.take() {
        ctx.withdraw(id);
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "hub",
            include_str!("../settings.toml"),
        );
    }

    #[test]
    fn sizes_stay_in_range() {
        let table = |text: &str| mochi_core::toml::from_str(text).unwrap();
        assert!(super::Settings::load(&table("height = 600")).is_ok());
        assert!(super::Settings::load(&table("height = 100")).is_err());
        assert!(super::Settings::load(&table("width = 5000")).is_err());
    }
}

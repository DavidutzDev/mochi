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
//! `mochi ipc hub toggle`, bound to a key, opens and closes it. The hub
//! takes the height its content needs, up to `height`; what's taller
//! scrolls.
//!
//! The home is editable: the pencil in the navbar lets you drag cards into
//! another order, take them off and put them back. Done sends `arrange`,
//! which keeps the result as the `order` and `hidden` settings. Those two
//! are live settings, so the hub stays open while they change.

mod tour;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, ContributionSpec, Module,
    ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority, SettingsOp,
};
use serde::Deserialize;
use serde_json::json;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Hub;

#[derive(Debug, Clone, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// The panel's width, in logical pixels.
    width: u32,
    /// The room for the cards or a page, above the navbar.
    height: u32,
    /// The home's cards in this order, as module/id like
    /// "network/status"; the ones not listed follow in their own order.
    #[schemars(extend("x-source" = "hub-card"))]
    order: Vec<String>,
    /// Cards to leave off the home, as module/id.
    #[schemars(extend("x-source" = "hub-card"))]
    hidden: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 860,
            height: 480,
            order: Vec::new(),
            hidden: Vec::new(),
        }
    }
}

/// Applied while the hub runs, so arranging the home keeps it open.
const LIVE: [&str; 2] = ["order", "hidden"];

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

    fn live_settings(&self) -> &'static [&'static str] {
        &LIVE
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("toggle", "Open the hub, or close it when open"),
            ActionSpec::new("open", "Open the hub").arg(
                ArgSpec::string("page", "A page as module/id, like notifications/history")
                    .optional()
                    .source("hub-page"),
            ),
            ActionSpec::new("close", "Close the hub"),
            ActionSpec::new(
                "arrange",
                "Keep the home's cards in an order, and hide some",
            )
            .arg(ArgSpec::string(
                "order",
                "Cards as module/id, separated by commas, like network/status,hub/clock",
            ))
            .arg(ArgSpec::string("hidden", "Cards to hide, the same way").optional()),
        ]
    }

    // The hub's own card goes through the same door as everyone else's.
    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("hub", "card", "clock", "Clock", "Today")
                .icon("clock")
                .order(3)
                .options(json!({ "span": 1, "rows": 1 })),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused sizes out of range.
            let mut settings: Settings = ctx.settings()?;
            let mut shown: Option<ActivityId> = None;
            publish(&ctx, &settings);
            while let Some(event) = ctx.next_event().await {
                match event {
                    ModuleEvent::Command(incoming) => {
                        command(&ctx, &mut settings, &mut shown, incoming);
                    }
                    ModuleEvent::Ended { activity, .. } if shown == Some(activity) => {
                        shown = None;
                    }
                    ModuleEvent::Reconfigured(table) => match Settings::load(&table) {
                        Ok(new) => {
                            settings = new;
                            publish(&ctx, &settings);
                        }
                        Err(error) => tracing::warn!(%error, "the hub's new settings"),
                    },
                    _ => {}
                }
            }
            Ok(())
        })
    }
}

/// The home's arrangement, for the view.
fn publish(ctx: &ModuleCtx, settings: &Settings) {
    ctx.publish_state(json!({
        "order": settings.order,
        "hidden": settings.hidden,
    }));
}

/// A list of cards as `arrange` takes it.
fn cards(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|card| !card.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Keeps an arrangement: shown at once, and written as changes, which come
/// back as `Reconfigured`.
fn arrange(ctx: &ModuleCtx, settings: &mut Settings, order: Vec<String>, hidden: Vec<String>) {
    settings.order = order;
    settings.hidden = hidden;
    publish(ctx, settings);
    for (key, value) in [("order", &settings.order), ("hidden", &settings.hidden)] {
        let set = ctx.settings_op(SettingsOp::Set {
            path: format!("config.module.hub.{key}"),
            value: json!(value),
        });
        tokio::spawn(async move {
            if let Err(error) = set.await {
                tracing::warn!(%error, "can't keep the hub's arrangement");
            }
        });
    }
}

fn command(
    ctx: &ModuleCtx,
    settings: &mut Settings,
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
        "arrange" => {
            let order = cards(command.args.str("order").unwrap_or_default());
            let hidden = cards(command.args.str("hidden").unwrap_or_default());
            arrange(ctx, settings, order, hidden);
            Ok(())
        }
        other => Err(format!("hub has no action {other}")),
    };
    command.reply(result);
}

fn open(ctx: &ModuleCtx, settings: &Settings, shown: &mut Option<ActivityId>, page: &str) {
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

    #[test]
    fn arrangements_are_lists_of_cards() {
        assert_eq!(
            super::cards(" network/status, hub/clock,,"),
            ["network/status", "hub/clock"]
        );
        assert!(super::cards("").is_empty());
    }
}

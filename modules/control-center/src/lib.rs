//! The control center: a wide panel that grows out of the island. Its home
//! shows cards and a navbar at the bottom switches to pages. Other modules
//! provide both through [`ContributionSpec`]s with `target = "control-center"`:
//!
//! - `card`: a tile on the home screen. `options.span` sets its width in
//!   columns, 1 to 3. Its heading, and a click beside its controls, open
//!   its module's page: `options.page` names one when it has several. A
//!   card with `options.spare` set to true starts off the home, under More
//!   cards while arranging it, until `order` lists it.
//! - `page`: a tab in the navbar, filling the panel when picked.
//!
//! Contributed views get their module's published state as `payload`, and fill
//! the space the control center gives them. The control center knows nothing
//! about them: a module that isn't enabled simply contributes nothing.
//!
//! `mochi ipc control-center toggle`, bound to a key, opens and closes it. The
//! control center takes the height its content needs, up to `height`; what's
//! taller scrolls.
//!
//! The home is editable: the pencil in the navbar, or `edit`, lets you drag
//! cards into another order, take them off and put them back, and the same
//! with the navbar's pages. Done sends `arrange` and `arrange-pages`, which
//! keep the result as the `order` and `hidden` settings, and `pages` and
//! `hidden_pages`. Those are live settings, so the control center stays
//! open while they change. A page left out of the navbar still opens by
//! name and from its module's card.

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
pub struct ControlCenter;

#[derive(Debug, Clone, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// The panel's width, in logical pixels.
    width: u32,
    /// The room for the cards or a page, above the navbar.
    height: u32,
    /// The home's cards in this order, as module/id like
    /// "network/status"; the ones not listed follow in their own order.
    #[schemars(extend("x-source" = "control-center-card"))]
    order: Vec<String>,
    /// Cards to leave off the home, as module/id.
    #[schemars(extend("x-source" = "control-center-card"))]
    hidden: Vec<String>,
    /// The navbar's pages in this order after Home, as module/id like
    /// "network/page"; the ones not listed follow in their own order.
    #[schemars(extend("x-source" = "control-center-page"))]
    pages: Vec<String>,
    /// Pages to leave out of the navbar, as module/id. They still open by
    /// name, with `mochi ipc control-center open`, and from their cards.
    #[schemars(extend("x-source" = "control-center-page"))]
    hidden_pages: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 860,
            height: 480,
            order: Vec::new(),
            hidden: Vec::new(),
            pages: Vec::new(),
            hidden_pages: Vec::new(),
        }
    }
}

/// Applied while the control center runs, so arranging the home keeps it open.
const LIVE: [&str; 4] = ["order", "hidden", "pages", "hidden_pages"];

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

impl Module for ControlCenter {
    fn id(&self) -> &'static str {
        "control-center"
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
            ActionSpec::new("toggle", "Open the control center, or close it when open"),
            ActionSpec::new("open", "Open the control center").arg(
                ArgSpec::string("page", "A page as module/id, like notifications/history")
                    .optional()
                    .source("control-center-page"),
            ),
            ActionSpec::new("close", "Close the control center"),
            ActionSpec::new(
                "edit",
                "Open the control center arranging its cards and pages",
            ),
            ActionSpec::new(
                "arrange",
                "Keep the home's cards in an order, and hide some",
            )
            .arg(ArgSpec::string(
                "order",
                "Cards as module/id, separated by commas, like network/status,control-center/clock",
            ))
            .arg(ArgSpec::string("hidden", "Cards to hide, the same way").optional()),
            ActionSpec::new(
                "arrange-pages",
                "Keep the navbar's pages in an order, and hide some",
            )
            .arg(ArgSpec::string(
                "pages",
                "Pages as module/id, separated by commas, like weather/page,network/page",
            ))
            .arg(ArgSpec::string("hidden", "Pages to hide, the same way").optional()),
        ]
    }

    // The control center's own card goes through the same door as everyone
    // else's.
    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("control-center", "card", "clock", "Clock", "Today")
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
                        Err(error) => tracing::warn!(%error, "the control center's new settings"),
                    },
                    _ => {}
                }
            }
            Ok(())
        })
    }
}

/// The home's and the navbar's arrangement, for the view.
fn publish(ctx: &ModuleCtx, settings: &Settings) {
    ctx.publish_state(json!({
        "order": settings.order,
        "hidden": settings.hidden,
        "pages": settings.pages,
        "hidden_pages": settings.hidden_pages,
    }));
}

/// A list of cards or pages as `arrange` and `arrange-pages` take it.
fn cards(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|card| !card.is_empty())
        .map(str::to_owned)
        .collect()
}

/// What an arrangement orders and hides.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Arranged {
    Cards,
    Pages,
}

/// Keeps an arrangement of the cards or the pages: shown at once, and
/// written as changes, which come back as `Reconfigured`.
fn arrange(
    ctx: &ModuleCtx,
    settings: &mut Settings,
    what: Arranged,
    order: Vec<String>,
    hidden: Vec<String>,
) {
    let kept = match what {
        Arranged::Cards => {
            settings.order = order;
            settings.hidden = hidden;
            [("order", &settings.order), ("hidden", &settings.hidden)]
        }
        Arranged::Pages => {
            settings.pages = order;
            settings.hidden_pages = hidden;
            [
                ("pages", &settings.pages),
                ("hidden_pages", &settings.hidden_pages),
            ]
        }
    };
    let kept = kept.map(|(key, value)| (key, value.clone()));
    publish(ctx, settings);
    for (key, value) in kept {
        let set = ctx.settings_op(SettingsOp::Set {
            path: format!("config.module.control-center.{key}"),
            value: json!(value),
        });
        tokio::spawn(async move {
            if let Err(error) = set.await {
                tracing::warn!(%error, "can't keep the control center's arrangement");
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
                false,
            );
            Ok(())
        }
        "edit" => {
            open(ctx, settings, shown, "home", true);
            Ok(())
        }
        "close" => {
            close(ctx, shown);
            Ok(())
        }
        "arrange" => {
            let order = cards(command.args.str("order").unwrap_or_default());
            let hidden = cards(command.args.str("hidden").unwrap_or_default());
            arrange(ctx, settings, Arranged::Cards, order, hidden);
            Ok(())
        }
        "arrange-pages" => {
            let order = cards(command.args.str("pages").unwrap_or_default());
            let hidden = cards(command.args.str("hidden").unwrap_or_default());
            arrange(ctx, settings, Arranged::Pages, order, hidden);
            Ok(())
        }
        other => Err(format!("control-center has no action {other}")),
    };
    command.reply(result);
}

/// Shows the control center on `page`, or on the home being arranged when
/// `editing`.
fn open(
    ctx: &ModuleCtx,
    settings: &Settings,
    shown: &mut Option<ActivityId>,
    page: &str,
    editing: bool,
) {
    ctx.close_other_panels();

    let spec = ActivitySpec::new("ControlCenter")
        .key("control-center")
        .priority(Priority::URGENT)
        .uninterruptible()
        .modal()
        .payload(json!({
            // Only the island on this monitor takes the keyboard.
            "output": ctx.compositor().state().focused_output,
            "page": page,
            "editing": editing,
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
            "control-center",
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
            super::cards(" network/status, control-center/clock,,"),
            ["network/status", "control-center/clock"]
        );
        assert!(super::cards("").is_empty());
    }
}

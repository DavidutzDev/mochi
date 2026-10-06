//! Battery: the laptop's battery from UPower.
//!
//! Dropping past a level on battery, 80, 50, 20 and 10 by default, shows a
//! short notice on the island, once per discharge. At or under the warning
//! level a bubble stays next to the island with the level, red at the
//! critical one, where the notice also stays longer. Plugging the charger
//! in or out shows a notice too. The hub has a card with the level and the
//! time left. Without a battery, as on a desktop, the module shows nothing.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.battery]
//! notices = [80, 50, 20, 10]   # levels that show a notice when passed
//! warning = 50                 # the warning bubble at or under this
//! critical = 10                # red, and a longer notice
//! plugged = true               # a notice on plugging in or out
//! ```

mod model;
mod upower;

use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivitySpec, Area, Assets, BoxFuture, BubbleId, BubbleSpec, ContributionSpec,
    Module, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use zbus::Connection;

use crate::model::{Levels, Notice, Tracker};
use crate::upower::Battery;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

const NOTICE: Duration = Duration::from_millis(3000);
const CRITICAL_NOTICE: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct BatteryModule;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    notices: Vec<u32>,
    warning: u32,
    critical: u32,
    plugged: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notices: vec![80, 50, 20, 10],
            warning: 50,
            critical: 10,
            plugged: true,
        }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        for (name, level) in [
            ("warning", settings.warning),
            ("critical", settings.critical),
        ]
        .into_iter()
        .chain(settings.notices.iter().map(|level| ("notices", *level)))
        {
            if level > 100 {
                return Err(format!("{name} has {level}; levels go from 0 to 100"));
            }
        }
        Ok(settings)
    }

    fn levels(&self) -> Levels {
        Levels {
            notices: self.notices.clone(),
            warning: self.warning,
            critical: self.critical,
            plugged: self.plugged,
        }
    }
}

impl Module for BatteryModule {
    fn id(&self) -> &'static str {
        "battery"
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
            ContributionSpec::new("hub", "card", "level", "Card", "Battery")
                .icon("bolt")
                .order(17)
                .options(json!({ "span": 1 })),
            // The same card on the desktop; it steps aside without a battery.
            ContributionSpec::new("widgets", "widget", "level", "Card", "Battery")
                .icon("bolt")
                .options(json!({ "size": [16, 6], "min": [12, 5], "max": [30, 10] })),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        Vec::new()
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused levels out of range.
            let settings: Settings = ctx.settings()?;
            let levels = settings.levels();
            let connection = Connection::system().await?;
            let (sender, mut batteries) = mpsc::unbounded_channel();
            tokio::spawn(upower::watch(connection, sender));
            let mut tracker = Tracker::default();
            let mut bubble: Option<BubbleId> = None;
            let mut was_critical = false;
            ctx.publish_state(model::payload(None, &levels));
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            command.reply(Err("battery has no actions".into()));
                        }
                        Some(ModuleEvent::BubbleClicked(_)) => {
                            let open = ctx.call("hub", "open", &[]);
                            tokio::spawn(async move {
                                if let Err(error) = open.await {
                                    tracing::debug!(%error, "can't open the hub");
                                }
                            });
                        }
                        Some(_) => {}
                    },
                    Some(battery) = batteries.recv() => {
                        ctx.publish_state(model::payload(battery.as_ref(), &levels));
                        if let Some(battery) = battery
                            && let Some(notice) = tracker.apply(battery, &levels)
                        {
                            show(&ctx, &notice);
                        }
                        update_bubble(&ctx, &mut bubble, &mut was_critical, battery.as_ref(), &levels);
                    }
                }
            }
        })
    }
}

fn show(ctx: &ModuleCtx, notice: &Notice) {
    let spec = ActivitySpec::new("Notice").key("notice").payload(json!({
        "text": notice.text,
        "level": notice.level,
        "charging": notice.charging,
        "critical": notice.critical,
    }));
    let spec = if notice.critical {
        // Not fleeting: it waits for a panel to close, and stays.
        spec.priority(Priority::URGENT).timeout(CRITICAL_NOTICE)
    } else {
        spec.priority(Priority::HIGH)
            .passive()
            .fleeting()
            .timeout(NOTICE)
    };
    ctx.present(spec);
}

fn update_bubble(
    ctx: &ModuleCtx,
    bubble: &mut Option<BubbleId>,
    was_critical: &mut bool,
    battery: Option<&Battery>,
    levels: &Levels,
) {
    match battery.and_then(|battery| model::bubble(battery, levels)) {
        Some(payload) => {
            let critical = payload["critical"] == true;
            let spec = BubbleSpec::new("Bubble")
                .key("warning")
                .area(Area::Right)
                .group("status")
                .payload(payload);
            // Turning critical is news; a percent less isn't.
            let spec = if bubble.is_some() && critical && !*was_critical {
                spec.news()
            } else {
                spec
            };
            *was_critical = critical;
            *bubble = Some(ctx.show_bubble(spec));
        }
        None => {
            if let Some(id) = bubble.take() {
                ctx.hide_bubble(id);
            }
        }
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "battery",
            include_str!("../settings.toml"),
        );
    }

    #[test]
    fn levels_stay_in_range() {
        let table = |text: &str| mochi_core::toml::from_str(text).unwrap();
        assert!(super::Settings::load(&table("warning = 30")).is_ok());
        assert!(super::Settings::load(&table("critical = 101")).is_err());
        assert!(super::Settings::load(&table("notices = [90, 120]")).is_err());
    }
}

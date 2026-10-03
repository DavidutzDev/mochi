//! Demo views and actions for trying the island and the arbiter's rules from
//! the command line. Enable it with `modules = ["idle", "demo"]` in
//! `config.toml` or `mochid --modules idle,demo`.
//!
//! ```sh
//! mochi ipc demo show Card "Some text"   # normal priority, waits its turn
//! mochi ipc demo alert Small             # high priority, interrupts
//! mochi ipc demo stack Wide              # same priority, interrupts
//! mochi ipc demo volume 40               # replaces the previous volume
//! mochi ipc demo clear                   # removes every demo activity
//! ```

use std::collections::HashSet;
use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivitySpec, ArgSpec, Args, Assets, BoxFuture, Module, ModuleCtx, ModuleError,
    ModuleEvent, Priority, SamePriority,
};
use serde_json::json;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

const VIEWS: [&str; 4] = ["Small", "Wide", "Card", "Big"];
const TIMEOUT: Duration = Duration::from_secs(4);
const VOLUME_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Default)]
pub struct Demo;

impl Module for Demo {
    fn id(&self) -> &'static str {
        "demo"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let view = || ArgSpec::choice("view", "Which view to show", VIEWS);
        let text = || {
            ArgSpec::string("text", "Text for the view")
                .optional()
                .rest()
        };
        vec![
            ActionSpec::new(
                "show",
                "Show a view at normal priority. It waits for anything already shown.",
            )
            .arg(view())
            .arg(text()),
            ActionSpec::new(
                "alert",
                "Show a view at high priority. It interrupts normal activities.",
            )
            .arg(view())
            .arg(text()),
            ActionSpec::new(
                "stack",
                "Show a view at normal priority that interrupts the one shown.",
            )
            .arg(view())
            .arg(text()),
            ActionSpec::new(
                "volume",
                "Show a volume level. A newer one replaces it in place.",
            )
            .arg(ArgSpec::int("level", "Volume from 0 to 100")),
            ActionSpec::new("clear", "Remove every demo activity."),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // Activities still shown or waiting, so `clear` can withdraw them.
            let mut live = HashSet::new();

            while let Some(event) = ctx.next_event().await {
                let command = match event {
                    ModuleEvent::Command(command) => command,
                    ModuleEvent::Ended { activity, .. } => {
                        live.remove(&activity);
                        continue;
                    }
                    ModuleEvent::Clicked(_) => continue,
                };

                let result = match command.action.as_str() {
                    "show" => Ok(view(&command.args)),
                    "alert" => Ok(view(&command.args).priority(Priority::HIGH)),
                    "stack" => Ok(view(&command.args).same_priority(SamePriority::Stack)),
                    "volume" => volume(&command.args),
                    "clear" => {
                        for activity in live.drain() {
                            ctx.withdraw(activity);
                        }
                        command.reply(Ok(()));
                        continue;
                    }
                    other => Err(format!("demo has no action {other}")),
                };

                match result {
                    Ok(spec) => {
                        live.insert(ctx.present(spec));
                        command.reply(Ok(()));
                    }
                    Err(message) => command.reply(Err(message)),
                }
            }
            Ok(())
        })
    }
}

fn view(args: &Args) -> ActivitySpec {
    let view = args.str("view").unwrap_or("Small");
    let mut spec = ActivitySpec::new(view)
        .timeout(TIMEOUT)
        .payload(json!({ "text": args.str("text").unwrap_or_default() }));
    if view == "Card" {
        spec = spec.expanded("CardExpanded");
    }
    spec
}

fn volume(args: &Args) -> Result<ActivitySpec, String> {
    let level = args.int("level").unwrap_or_default();
    if !(0..=100).contains(&level) {
        return Err(format!("level must be between 0 and 100, got {level}"));
    }
    Ok(ActivitySpec::new("Volume")
        .key("volume")
        .priority(Priority::HIGH)
        .timeout(VOLUME_TIMEOUT)
        .payload(json!({ "level": level })))
}

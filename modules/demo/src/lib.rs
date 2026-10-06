//! Demo views and actions for trying the island and the arbiter's rules from
//! the command line. Enable it with `modules = ["idle", "demo"]` in
//! `config.toml` or `mochid --modules idle,demo`.
//!
//! ```sh
//! mochi ipc demo show Card "Some text"   # normal priority, waits its turn
//! mochi ipc demo alert Small             # high priority, interrupts
//! mochi ipc demo stack Wide              # same priority, interrupts
//! mochi ipc demo volume 40               # replaces the previous volume
//! mochi ipc demo bubble wifi right       # a bubble named wifi on the right
//! mochi ipc demo bubble bt right status  # joins the status pill there
//! mochi ipc demo pop wifi                # removes that bubble
//! mochi ipc demo clear                   # removes every demo activity and bubble
//! mochi ipc demo call launcher open      # runs another module's action
//! mochi ipc demo controls                # every built-in control, to try
//! ```
//!
//! Clicking a demo bubble shows its name on the island.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivitySpec, Area, ArgSpec, Args, Assets, BoxFuture, BubbleSpec, Module,
    ModuleCtx, ModuleError, ModuleEvent, Priority, SamePriority,
};
use serde_json::json;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

const VIEWS: [&str; 4] = ["Small", "Wide", "Card", "Big"];
const AREAS: [&str; 5] = ["left", "center-left", "center", "center-right", "right"];
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
            ActionSpec::new(
                "bubble",
                "Show a bubble, or move the one with the same name.",
            )
            .arg(ArgSpec::string("name", "Text in the bubble, and its key"))
            .arg(ArgSpec::choice("area", "Where it goes", AREAS))
            .arg(ArgSpec::string("group", "Bubbles with the same group share a pill").optional()),
            ActionSpec::new("pop", "Remove a bubble by name.")
                .arg(ArgSpec::string("name", "The bubble's name")),
            ActionSpec::new("clear", "Remove every demo activity and bubble."),
            ActionSpec::new("controls", "Show every built-in control, for 30 seconds."),
            ActionSpec::new(
                "call",
                "Run another module's action from this module, to try calls.",
            )
            .arg(ArgSpec::string("module", "The module to call"))
            .arg(ArgSpec::string("action", "Its action"))
            .arg(
                ArgSpec::string("args", "The action's arguments")
                    .optional()
                    .rest(),
            ),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // Activities still shown or waiting, so `clear` can withdraw them.
            let mut live = HashSet::new();
            let mut bubbles = HashMap::new();

            while let Some(event) = ctx.next_event().await {
                let command = match event {
                    ModuleEvent::Command(command) => command,
                    ModuleEvent::Ended { activity, .. } => {
                        live.remove(&activity);
                        continue;
                    }
                    ModuleEvent::Clicked(_)
                    | ModuleEvent::State { .. }
                    | ModuleEvent::Offers(_) => {
                        continue;
                    }
                    ModuleEvent::BubbleClicked(clicked) => {
                        if let Some((name, _)) = bubbles.iter().find(|(_, id)| **id == clicked) {
                            let spec = ActivitySpec::new("Small")
                                .timeout(TIMEOUT)
                                .payload(json!({ "text": format!("{name} clicked") }));
                            live.insert(ctx.present(spec));
                        }
                        continue;
                    }
                };

                let result = match command.action.as_str() {
                    "show" => Ok(view(&command.args)),
                    "alert" => Ok(view(&command.args).priority(Priority::HIGH)),
                    "stack" => Ok(view(&command.args).same_priority(SamePriority::Stack)),
                    "volume" => volume(&command.args),
                    "controls" => Ok(ActivitySpec::new("Controls")
                        .key("controls")
                        .timeout(Duration::from_secs(30))),
                    "call" => {
                        let module = command.args.str("module").unwrap_or_default();
                        let action = command.args.str("action").unwrap_or_default();
                        let args: Vec<&str> = command
                            .args
                            .str("args")
                            .map(|args| args.split_whitespace().collect())
                            .unwrap_or_default();
                        // Spawned: the called module may be slow, or call back.
                        let call = ctx.call(module, action, &args);
                        tokio::spawn(async move {
                            command.reply(call.await.map_err(|error| error.to_string()));
                        });
                        continue;
                    }
                    "bubble" => {
                        let name = command.args.str("name").unwrap_or_default().to_owned();
                        let id = ctx.show_bubble(bubble(&name, &command.args));
                        bubbles.insert(name, id);
                        command.reply(Ok(()));
                        continue;
                    }
                    "pop" => {
                        let name = command.args.str("name").unwrap_or_default();
                        let result = match bubbles.remove(name) {
                            Some(id) => {
                                ctx.hide_bubble(id);
                                Ok(())
                            }
                            None => Err(format!("no bubble named {name:?}")),
                        };
                        command.reply(result);
                        continue;
                    }
                    "clear" => {
                        for activity in live.drain() {
                            ctx.withdraw(activity);
                        }
                        for (_, id) in bubbles.drain() {
                            ctx.hide_bubble(id);
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

fn bubble(name: &str, args: &Args) -> BubbleSpec {
    // The argument parser already checked the choice.
    let area: Area =
        serde_json::from_value(json!(args.str("area").unwrap_or("right"))).unwrap_or_default();
    let mut spec = BubbleSpec::new("Bubble")
        .wide("BubbleWide")
        .key(name)
        .area(area)
        .payload(json!({ "text": name }));
    if let Some(group) = args.str("group") {
        spec = spec.group(group);
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

//! The hub: a wide panel that grows out of the island. Its home shows cards
//! and a navbar at the bottom switches to pages. Other modules provide both
//! through [`ContributionSpec`]s with `target = "hub"`:
//!
//! - `card`: a tile on the home screen. `options.span` sets its width in
//!   columns, 1 to 3.
//! - `page`: a tab in the navbar, filling the panel when picked.
//!
//! Contributed views get their module's published state as `payload`, and
//! fill the space the hub gives them. The hub knows nothing about them: a
//! module that isn't enabled simply contributes nothing.
//!
//! `mochi ipc hub toggle`, bound to a key, opens and closes it.

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, ContributionSpec,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde_json::json;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Hub;

impl Module for Hub {
    fn id(&self) -> &'static str {
        "hub"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
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
                .options(json!({ "span": 1 })),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let mut shown: Option<ActivityId> = None;
            while let Some(event) = ctx.next_event().await {
                match event {
                    ModuleEvent::Command(incoming) => command(&ctx, &mut shown, incoming),
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

fn command(ctx: &ModuleCtx, shown: &mut Option<ActivityId>, command: ModuleCommand) {
    let result = match command.action.as_str() {
        "toggle" if shown.is_some() => {
            close(ctx, shown);
            Ok(())
        }
        "toggle" | "open" => {
            open(ctx, shown, command.args.str("page").unwrap_or("home"));
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

fn open(ctx: &ModuleCtx, shown: &mut Option<ActivityId>, page: &str) {
    // The launcher and the clipboard take the keyboard too; only one can be
    // open. Not awaited: they close the hub the same way.
    for module in ["launcher", "clipboard"] {
        let close = ctx.call(module, "close", &[]);
        tokio::spawn(async move {
            match close.await {
                Ok(()) | Err(CallError::NotEnabled(_)) => {}
                Err(error) => tracing::warn!(%error, module, "could not close it"),
            }
        });
    }

    let spec = ActivitySpec::new("Hub")
        .key("hub")
        .priority(Priority::URGENT)
        .uninterruptible()
        .modal()
        .payload(json!({
            // Only the island on this monitor takes the keyboard.
            "output": ctx.compositor().state().focused_output,
            "page": page,
        }));
    *shown = Some(ctx.present(spec));
}

fn close(ctx: &ModuleCtx, shown: &mut Option<ActivityId>) {
    if let Some(id) = shown.take() {
        ctx.withdraw(id);
    }
}

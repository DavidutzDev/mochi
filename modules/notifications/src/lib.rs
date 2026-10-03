//! The notification daemon: apps send notifications over D-Bus
//! (`org.freedesktop.Notifications`) and they pop up on the island.
//!
//! - A popup shows the app icon or picture, the summary and the body.
//!   Clicking it expands it: the whole text and the app's action buttons.
//! - A new popup from an app replaces that app's popup in place, so a burst
//!   of messages shows only the latest; the others count as missed. Popups
//!   from different apps stack: the newest on top, older ones after it.
//! - Critical ones, like a low battery, stay until closed, and nothing
//!   interrupts them.
//! - Popups that time out go to the history. A bubble counts them; clicking
//!   it lists them on the island.
//! - Do not disturb sends everything but critical ones straight to the
//!   history, and shows a bubble while it's on.
//!
//! If another notification daemon runs, Mochi waits for the name and takes
//! over when that daemon stops.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.notifications]
//! timeout_ms = 5000     # how long a popup stays, unless the app says
//! history = 50          # missed notifications kept
//! same_app = "replace"  # or "stack": every popup from an app in turn
//!
//! [bubbles.notifications]  # the missed count; center-right by default
//! area = "right"
//! ```

mod center;
mod note;
mod server;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    EndReason, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority, SamePriority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use zbus::Connection;

use crate::center::{Center, Effect, PopupEnd, Reason};
use crate::note::{Image, Note, Urgency};
use crate::server::Incoming;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The count and do-not-disturb bubbles share one pill.
const GROUP: &str = "notifications";
const HISTORY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct Notifications;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    timeout_ms: u64,
    history: usize,
    same_app: SameApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SameApp {
    /// The latest popup from an app replaces the one shown.
    Replace,
    /// Every popup from an app shows in turn.
    Stack,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            history: 50,
            same_app: SameApp::Replace,
        }
    }
}

impl Module for Notifications {
    fn id(&self) -> &'static str {
        "notifications"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let id = || ArgSpec::int("id", "The notification's id");
        vec![
            ActionSpec::new("history", "List the missed notifications on the island"),
            ActionSpec::new("clear", "Remove every missed notification"),
            ActionSpec::new("dnd", "Do not disturb: only critical notifications pop up").arg(
                ArgSpec::choice(
                    "state",
                    "Turn it on, off, or flip it",
                    ["on", "off", "toggle"],
                ),
            ),
            ActionSpec::new("dismiss", "Close a notification").arg(id()),
            ActionSpec::new("invoke", "Run one of a notification's actions")
                .arg(id())
                .arg(ArgSpec::string("action", "The action's key, like default")),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let (sender, mut incoming) = mpsc::unbounded_channel();
            let connection = server::start(sender).await?;
            let mut daemon = Daemon::new(&settings, ctx.data_dir().to_owned(), connection);

            loop {
                tokio::select! {
                    message = incoming.recv() => match message {
                        Some(Incoming::Notify(note)) => {
                            let note = daemon.store_image(*note);
                            let effects = daemon.center.notify(note);
                            daemon.apply(&ctx, effects).await;
                        }
                        Some(Incoming::Close(id)) => {
                            let effects = daemon.center.close(id, Reason::Closed);
                            daemon.apply(&ctx, effects).await;
                        }
                        None => return Ok(()),
                    },
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => daemon.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, reason }) => {
                            daemon.ended(&ctx, activity, reason).await;
                        }
                        Some(ModuleEvent::BubbleClicked(_)) => daemon.show_history(&ctx),
                        Some(ModuleEvent::Clicked(_)) => {}
                    },
                }
                daemon.sync(&ctx);
            }
        })
    }
}

/// The module's state around the [`Center`]: which activity and bubble show
/// what, and the image files written for notifications.
#[derive(Debug)]
struct Daemon {
    center: Center,
    timeout: Duration,
    connection: Connection,
    data_dir: PathBuf,
    /// Image files written from pixels, by notification.
    images: BTreeMap<u32, PathBuf>,
    /// Numbers image files, so a replaced image gets a new URL and the UI
    /// doesn't show its cached copy.
    written: u64,
    popups: BTreeMap<u32, ActivityId>,
    history_view: Option<ActivityId>,
    count_bubble: Option<(BubbleId, usize)>,
    dnd_bubble: Option<BubbleId>,
}

impl Daemon {
    fn new(settings: &Settings, data_dir: PathBuf, connection: Connection) -> Self {
        Self {
            center: Center::new(settings.history, settings.same_app == SameApp::Replace),
            timeout: Duration::from_millis(settings.timeout_ms),
            connection,
            data_dir,
            images: BTreeMap::new(),
            written: 0,
            popups: BTreeMap::new(),
            history_view: None,
            count_bubble: None,
            dnd_bubble: None,
        }
    }

    /// Views load files, so raw pixels become a PNG in the data directory.
    fn store_image(&mut self, mut note: Note) -> Note {
        if let Some(Image::Pixels {
            width,
            height,
            rgba,
        }) = &note.image
        {
            self.written += 1;
            let path = self
                .data_dir
                .join(format!("{}-{}.png", note.id, self.written));
            note.image = match write_png(&path, *width, *height, rgba) {
                Ok(()) => Some(Image::Path(path.display().to_string())),
                Err(error) => {
                    tracing::warn!(%error, "could not write a notification's image");
                    None
                }
            };
            if let Some(old) = self.images.insert(note.id, path) {
                let _ = std::fs::remove_file(old);
            }
        }
        note
    }

    async fn apply(&mut self, ctx: &ModuleCtx, effects: Vec<Effect>) {
        for effect in effects {
            let result = match effect {
                Effect::Popup(id) => {
                    if let Some(note) = self.center.get(id) {
                        let activity = ctx.present(self.popup(note));
                        self.popups.insert(id, activity);
                    }
                    Ok(())
                }
                Effect::Withdraw(id) => {
                    if let Some(activity) = self.popups.remove(&id) {
                        ctx.withdraw(activity);
                    }
                    Ok(())
                }
                Effect::Closed(id, reason) => {
                    if let Some(path) = self.images.remove(&id) {
                        let _ = std::fs::remove_file(path);
                    }
                    server::closed(&self.connection, id, reason).await
                }
                // The next popup replaces it through the shared key.
                Effect::Superseded(id) => {
                    self.popups.remove(&id);
                    Ok(())
                }
                Effect::Invoked(id, action) => server::invoked(&self.connection, id, &action).await,
            };
            if let Err(error) = result {
                tracing::warn!(%error, "could not tell the app");
            }
        }
    }

    fn popup(&self, note: &Note) -> ActivitySpec {
        let spec = ActivitySpec::new("Compact")
            .expanded("Expanded")
            .key(if self.center.replaces_same_app(note) {
                format!("app-{}", note.app)
            } else {
                format!("notification-{}", note.id)
            })
            .payload(payload(note));
        match note.urgency {
            Urgency::Critical => spec.priority(Priority::URGENT).uninterruptible(),
            Urgency::Normal | Urgency::Low => spec
                .same_priority(SamePriority::Stack)
                .timeout(note.timeout.unwrap_or(self.timeout)),
        }
    }

    async fn ended(&mut self, ctx: &ModuleCtx, activity: ActivityId, reason: EndReason) {
        if self.history_view == Some(activity) {
            self.history_view = None;
            return;
        }
        let Some(id) = self
            .popups
            .iter()
            .find_map(|(id, popup)| (*popup == activity).then_some(*id))
        else {
            return;
        };
        let end = match reason {
            EndReason::Expired => PopupEnd::TimedOut,
            EndReason::Dismissed => PopupEnd::Dismissed,
            // Taken down or replaced on purpose: nothing to do.
            EndReason::Withdrawn | EndReason::Replaced => return,
        };
        self.popups.remove(&id);
        let effects = self.center.popup_ended(id, end);
        self.apply(ctx, effects).await;
    }

    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let id = || {
            command
                .args
                .int("id")
                .and_then(|id| u32::try_from(id).ok())
                .unwrap_or_default()
        };
        let result = match command.action.as_str() {
            "history" => {
                self.show_history(ctx);
                Ok(Vec::new())
            }
            "clear" => Ok(self.center.clear()),
            "dnd" => {
                let on = match command.args.str("state") {
                    Some("on") => true,
                    Some("off") => false,
                    _ => !self.center.dnd(),
                };
                self.center.set_dnd(on);
                Ok(Vec::new())
            }
            "dismiss" => match self.center.close(id(), Reason::Dismissed) {
                effects if effects.is_empty() => Err(format!("no notification {}", id())),
                effects => Ok(effects),
            },
            "invoke" => {
                let action = command.args.str("action").unwrap_or_default().to_owned();
                self.center.invoke(id(), &action)
            }
            other => Err(format!("notifications has no action {other}")),
        };
        match result {
            Ok(effects) => {
                self.apply(ctx, effects).await;
                command.reply(Ok(()));
            }
            Err(message) => command.reply(Err(message)),
        }
    }

    fn show_history(&mut self, ctx: &ModuleCtx) {
        let spec = ActivitySpec::new("History")
            .key("history")
            .same_priority(SamePriority::Stack)
            .timeout(HISTORY_TIMEOUT)
            .payload(self.history_payload());
        self.history_view = Some(ctx.present(spec));
    }

    fn history_payload(&self) -> Value {
        json!({
            "dnd": self.center.dnd(),
            "notes": self.center.history().map(payload).collect::<Vec<_>>(),
        })
    }

    /// Brings the bubbles and the history view in line with the center.
    fn sync(&mut self, ctx: &ModuleCtx) {
        let count = self.center.history_len();
        match self.count_bubble {
            Some((_, shown)) if shown == count => {}
            Some((id, _)) if count == 0 => {
                ctx.hide_bubble(id);
                self.count_bubble = None;
            }
            _ if count == 0 => {}
            _ => {
                let spec = bubble("Count", "count").payload(json!({ "count": count }));
                self.count_bubble = Some((ctx.show_bubble(spec), count));
            }
        }

        match (self.center.dnd(), self.dnd_bubble) {
            (true, None) => self.dnd_bubble = Some(ctx.show_bubble(bubble("Dnd", "dnd"))),
            (false, Some(id)) => {
                ctx.hide_bubble(id);
                self.dnd_bubble = None;
            }
            _ => {}
        }

        if let Some(view) = self.history_view {
            ctx.update(view, self.history_payload());
        }
    }
}

fn bubble(view: &str, key: &str) -> BubbleSpec {
    BubbleSpec::new(view)
        .key(key)
        .area(Area::CenterRight)
        .group(GROUP)
        .priority(Priority::LOW)
}

/// What the views get for one notification.
fn payload(note: &Note) -> Value {
    let image = match &note.image {
        Some(Image::Path(path)) if path.starts_with('/') => Some(format!("file://{path}")),
        Some(Image::Path(path)) => Some(path.clone()),
        _ => None,
    };
    let received = note
        .received
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    json!({
        "id": note.id,
        "app": note.app,
        "icon": note.icon,
        "image": image,
        "summary": note.summary,
        "body": note.body,
        // `default` is what a click on the text does, not a button.
        "actions": note
            .actions
            .iter()
            .filter(|action| action.key != "default")
            .map(|action| json!({ "key": action.key, "label": action.label }))
            .collect::<Vec<_>>(),
        "default": note.action("default").is_some(),
        "urgency": note.urgency.as_str(),
        "received_ms": u64::try_from(received.as_millis()).unwrap_or(u64::MAX),
    })
}

fn write_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(rgba)
        .map_err(std::io::Error::other)?;
    writer.finish().map_err(std::io::Error::other)
}

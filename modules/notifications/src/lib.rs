//! The notification daemon: apps send notifications over D-Bus
//! (`org.freedesktop.Notifications`) and they pop up on the island.
//!
//! - A popup shows the app icon or picture, the summary and the body.
//!   Clicking it expands it: the whole text and the app's action buttons.
//! - The body may have bold, italic, underline and links ([`markup`]). A
//!   click on a link opens it with `xdg-open` and closes the notification.
//! - A notification with an `inline-reply` action gets a Reply button,
//!   which opens a text field; Enter sends the text back to the app.
//! - A new popup from an app replaces that app's popup in place, so a burst
//!   of messages shows only the latest; the others count as missed. Popups
//!   from different apps stack: the newest on top, older ones after it.
//! - Critical ones, like a low battery, stay until closed, and nothing
//!   interrupts them.
//! - Popups that time out go to the history. A bubble counts them; clicking
//!   it lists them on the island.
//! - Do not disturb sends everything but critical ones straight to the
//!   history, and shows a bubble while it's on.
//! - The history lasts across restarts, in `$XDG_STATE_HOME/mochi/`.
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
//! save_history = true   # keep the history across restarts
//!
//! [bubbles.notifications]  # the missed count; center-right by default
//! area = "right"
//! ```

mod center;
mod markdown;
mod markup;
mod note;
mod saved;
mod server;
mod tour;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    ContributionSpec, EndReason, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent,
    Priority, SamePriority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use zbus::Connection;

use crate::center::{Center, Effect, PopupEnd, Reason};
use crate::note::{Image, Note, REPLY, Urgency};
use crate::server::Incoming;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The count and do-not-disturb bubbles share one pill.
const GROUP: &str = "notifications";
const HISTORY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct Notifications;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    timeout_ms: u64,
    history: usize,
    same_app: SameApp,
    save_history: bool,
    markdown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
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
            save_history: true,
            markdown: true,
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

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("hub", "card", "missed", "Card", "Notifications")
                .icon("bell")
                .order(5)
                .options(json!({ "span": 1, "rows": 1 })),
            ContributionSpec::new("hub", "page", "history", "Page", "Notifications")
                .icon("bell")
                .order(20),
        ];
        offers.extend(tour::steps());
        offers
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
            ActionSpec::new("reply", "Answer a notification that takes a reply")
                .arg(id())
                .arg(ArgSpec::string("text", "The reply")),
            ActionSpec::new("open", "Open a link in a notification's text, and close it")
                .arg(id())
                .arg(ArgSpec::string("url", "The link, as in its href")),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let saved = saved::dir().filter(|_| settings.save_history);
            let mut center = Center::new(settings.history, settings.same_app == SameApp::Replace);
            // Images from pixels go next to the saved history, so they last
            // as long as it does.
            let images = match &saved {
                Some(dir) => {
                    let images = dir.join("notifications");
                    let restored = saved::load(&dir.join("notifications.json"));
                    if let Err(error) = std::fs::create_dir_all(&images) {
                        tracing::warn!(%error, "can't keep notification images");
                    }
                    saved::tidy(&images, &restored);
                    center.restore(restored);
                    images
                }
                None => ctx.data_dir().to_owned(),
            };
            let (sender, mut incoming) = mpsc::unbounded_channel();
            let connection = server::start(sender, center.last_id()).await?;
            let mut daemon = Daemon::new(
                &settings,
                center,
                images,
                saved.map(|dir| dir.join("notifications.json")),
                connection,
            );

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
                        Some(
                            ModuleEvent::Clicked(_)
                            | ModuleEvent::Hovered { .. }
                            | ModuleEvent::State { .. }
                            | ModuleEvent::Settings(_)
                            | ModuleEvent::Offers(_),
                        ) => {}
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
    /// Whether bodies' Markdown is read, see [`markdown`].
    markdown: bool,
    connection: Connection,
    /// Where images from pixels go.
    image_dir: PathBuf,
    /// The file the history is saved to, when it is.
    saved: Option<PathBuf>,
    /// Image files written from pixels, by notification.
    images: BTreeMap<u32, PathBuf>,
    /// Numbers image files, so a replaced image gets a new URL and the UI
    /// doesn't show its cached copy.
    written: u64,
    popups: BTreeMap<u32, ActivityId>,
    history_view: Option<ActivityId>,
    count_bubble: Option<(BubbleId, usize)>,
    dnd_bubble: Option<BubbleId>,
    /// The state last published, so unchanged history isn't sent again.
    published: Value,
}

impl Daemon {
    fn new(
        settings: &Settings,
        center: Center,
        image_dir: PathBuf,
        saved: Option<PathBuf>,
        connection: Connection,
    ) -> Self {
        // Restored images are deleted with their notification, like new ones.
        let images = center
            .history()
            .filter_map(|note| match &note.image {
                Some(Image::Path(path)) if Path::new(path).starts_with(&image_dir) => {
                    Some((note.id, PathBuf::from(path)))
                }
                _ => None,
            })
            .collect();
        Self {
            center,
            timeout: Duration::from_millis(settings.timeout_ms),
            markdown: settings.markdown,
            connection,
            image_dir,
            saved,
            images,
            written: 0,
            popups: BTreeMap::new(),
            history_view: None,
            count_bubble: None,
            dnd_bubble: None,
            published: Value::Null,
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
                .image_dir
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
                Effect::Replied(id, text) => server::replied(&self.connection, id, &text).await,
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
            .payload(payload(note, self.markdown));
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
            // A click elsewhere only moves it to the missed ones, as a timeout
            // would.
            EndReason::Expired | EndReason::Outside => PopupEnd::TimedOut,
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
            "reply" => {
                let text = command.args.str("text").unwrap_or_default().to_owned();
                self.center.reply(id(), &text)
            }
            "open" => self.open(id(), command.args.str("url").unwrap_or_default()),
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

    /// Opens a link from a notification's body in the browser, detached
    /// like the launcher's apps, and closes the notification. Only links
    /// the markup kept: the views can't open anything else through this.
    fn open(&mut self, id: u32, url: &str) -> Result<Vec<Effect>, String> {
        let note = self
            .center
            .get(id)
            .ok_or_else(|| format!("no notification {id}"))?;
        if !markup::links(&note.body, self.markdown)
            .iter()
            .any(|link| link == url)
        {
            return Err(format!("notification {id} has no link {url:?}"));
        }
        mochi_core::process::spawn_detached(
            &mochi_core::process::in_app_scope(&["xdg-open".into(), url.to_owned()]),
            None,
        )?;
        Ok(self.center.close(id, Reason::Dismissed))
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
            "notes": self
                .center
                .history()
                .map(|note| payload(note, self.markdown))
                .collect::<Vec<_>>(),
        })
    }

    /// Brings the bubbles and the history view in line with the center.
    fn sync(&mut self, ctx: &ModuleCtx) {
        // The history and do not disturb, for views outside the island like
        // the hub's card and page: the same shape the history view gets.
        let state = self.history_payload();
        if state != self.published {
            ctx.publish_state(state.clone());
            self.published = state;
            if let Some(path) = &self.saved {
                // The file wants the oldest first.
                let notes: Vec<&Note> = self.center.history().collect();
                if let Err(error) = saved::save(path, notes.into_iter().rev()) {
                    tracing::warn!(%error, "could not save the notification history");
                }
            }
        }

        let count = self.center.history_len();
        match self.count_bubble {
            Some((_, shown)) if shown == count => {}
            Some((id, _)) if count == 0 => {
                ctx.hide_bubble(id);
                self.count_bubble = None;
            }
            _ if count == 0 => {}
            Some((_, shown)) => {
                let spec = bubble("Count", "count").payload(json!({ "count": count }));
                // One more missed is news; one fewer isn't.
                let spec = if count > shown { spec.news() } else { spec };
                self.count_bubble = Some((ctx.show_bubble(spec), count));
            }
            None => {
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
fn payload(note: &Note, markdown: bool) -> Value {
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
        // StyledText: the whole body, and its first line for one-line rows.
        "body": markup::markup(&note.body, markdown),
        "line": markup::first_line(&note.body, markdown),
        // `default` is what a click on the text does, and `inline-reply`
        // opens a text field: neither is a plain button.
        "actions": note
            .actions
            .iter()
            .filter(|action| action.key != "default" && action.key != REPLY)
            .map(|action| json!({ "key": action.key, "label": action.label }))
            .collect::<Vec<_>>(),
        "default": note.action("default").is_some(),
        // The Reply button's label, when it takes a reply.
        "reply": note.action(REPLY).map(|action| {
            if action.label.trim().is_empty() { "Reply" } else { action.label.as_str() }
        }),
        "urgency": note.urgency.as_str(),
        "received_ms": u64::try_from(received.as_millis()).unwrap_or(u64::MAX),
    })
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
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

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "notifications",
            include_str!("../settings.toml"),
        );
    }
}

//! Clipboard history. The module watches the clipboard through
//! `ext-data-control-v1` and keeps every text and image copied, in a file
//! of its own (see [`store`]). `mochi ipc clipboard toggle`, bound to a key,
//! grows the island into a search over the history; picking an entry puts
//! it back on the clipboard and pastes it into the window you were in.
//!
//! The history stays in the runtime directory by default, a tmpfs gone at
//! logout. With `storage = "disk"` it's kept in `$XDG_STATE_HOME/mochi`,
//! encrypted with a key from the Secret Service. Copies password managers
//! mark as secret are never read.

mod search;
mod secret;
mod store;
mod wayland;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, ContributionSpec,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::store::{Clip, Kind, Store};
use crate::wayland::{Formats, Request, Watcher};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long the window takes to get the keyboard back once the island lets
/// go, before typing Ctrl+V into it.
const REFOCUS: Duration = Duration::from_millis(150);
/// How often old entries are looked for, when they expire.
const EXPIRY_CHECK: Duration = Duration::from_secs(600);
/// The most characters of a text the picker shows.
const SHOWN: usize = 300;

#[derive(Debug, Default)]
pub struct Clipboard;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Storage {
    #[default]
    Memory,
    Disk,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    storage: Storage,
    max_entries: usize,
    max_age_hours: u64,
    max_item_mb: usize,
    max_results: usize,
    paste: bool,
    terminals: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            storage: Storage::Memory,
            max_entries: 500,
            max_age_hours: 0,
            max_item_mb: 16,
            max_results: 50,
            paste: true,
            terminals: [
                "kitty",
                "foot",
                "footclient",
                "Alacritty",
                "com.mitchellh.ghostty",
                "org.wezfurlong.wezterm",
                "org.kde.konsole",
                "org.gnome.Console",
                "org.gnome.Ptyxis",
                "xterm",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

impl Module for Clipboard {
    fn id(&self) -> &'static str {
        "clipboard"
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

    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("hub", "card", "history", "Card", "Clipboard")
                .icon("clipboard")
                .order(30)
                .options(json!({ "span": 1 })),
            ContributionSpec::new("hub", "page", "history", "Page", "Clipboard")
                .icon("clipboard")
                .order(30),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let id = || ArgSpec::int("id", "The entry's id");
        vec![
            ActionSpec::new("toggle", "Open the history, or close it when open"),
            ActionSpec::new("open", "Open the history"),
            ActionSpec::new("close", "Close the history"),
            ActionSpec::new(
                "search",
                "Search the history; the picker sends this as you type",
            )
            .arg(
                ArgSpec::string("query", "What to look for")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new(
                "pick",
                "Copy an entry and paste it into the window you were in",
            )
            .arg(id()),
            ActionSpec::new("copy", "Copy an entry without pasting it").arg(id()),
            ActionSpec::new(
                "show",
                "Open an image entry in the preview card a screenshot gets",
            )
            .arg(id()),
            ActionSpec::new("delete", "Remove an entry from the history").arg(id()),
            ActionSpec::new("clear", "Remove everything from the history"),
            ActionSpec::new("pause", "Stop keeping what you copy, or start again").arg(
                ArgSpec::choice(
                    "state",
                    "Pause, resume, or flip it",
                    ["on", "off", "toggle"],
                ),
            ),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let mut state = State::load(&ctx, settings).await;
            let (sender, mut copied) = mpsc::unbounded_channel();
            state.clipboard = match wayland::start(
                sender,
                state.listening.clone(),
                state.settings.max_item_mb.saturating_mul(1 << 20),
            ) {
                Ok(clipboard) => Some(clipboard),
                Err(error) => {
                    tracing::warn!(%error, "can't watch the clipboard");
                    state.warning = Some(error);
                    None
                }
            };
            state.expire(&ctx);
            state.publish(&ctx);

            let mut watching = state.clipboard.is_some();
            let mut expiry = tokio::time::interval(EXPIRY_CHECK);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, .. }) if state.shown == Some(activity) => {
                            state.shown = None;
                        }
                        Some(_) => {}
                    },
                    clip = copied.recv(), if watching => match clip {
                        Some(clip) => state.copied(&ctx, &clip),
                        None => watching = false,
                    },
                    _ = expiry.tick(), if state.settings.max_age_hours > 0 => state.expire(&ctx),
                }
            }
        })
    }
}

#[derive(Debug)]
struct State {
    settings: Settings,
    store: Store,
    /// Where the history actually is: memory when the disk had no key.
    storage: Storage,
    clipboard: Option<Watcher>,
    /// False while paused.
    listening: Arc<AtomicBool>,
    /// Why something doesn't work, for the hub card.
    warning: Option<String>,
    /// Where images are written for the picker to show.
    pictures: PathBuf,
    written: HashSet<u64>,
    query: String,
    shown: Option<ActivityId>,
}

impl State {
    /// Opens the history where the settings say, or in memory when the disk
    /// has no key, or empty when even that fails.
    async fn load(ctx: &ModuleCtx, settings: Settings) -> Self {
        let memory = ctx.session_dir().join("history");
        let mut warning = None;
        let (path, key, storage) = match settings.storage {
            Storage::Memory => (memory.clone(), None, Storage::Memory),
            Storage::Disk => match (disk_path(), secret::key().await) {
                (Some(path), Ok(key)) => (path, Some(key), Storage::Disk),
                (None, _) => {
                    warning = Some("no home directory; kept in memory".to_owned());
                    (memory.clone(), None, Storage::Memory)
                }
                (_, Err(error)) => {
                    tracing::warn!(%error, "no key for the clipboard history; keeping it in memory");
                    warning = Some(format!("{error}; kept in memory"));
                    (memory.clone(), None, Storage::Memory)
                }
            },
        };
        let store = match Store::open(&path, key) {
            Ok(store) => store,
            Err(error) => {
                tracing::error!(%error, path = %path.display(), "cannot open the clipboard history");
                warning = Some(format!("cannot open the history: {error}"));
                // A fresh file next to it, so copying still works.
                let fallback = memory.with_file_name("history.fallback");
                let _ = std::fs::remove_file(&fallback);
                match Store::open(&fallback, None) {
                    Ok(store) => store,
                    Err(error) => panic!("cannot keep a clipboard history anywhere: {error}"),
                }
            }
        };
        tracing::info!(
            entries = store.entries().len(),
            storage = ?storage,
            "clipboard history ready"
        );
        Self {
            settings,
            store,
            storage,
            clipboard: None,
            listening: Arc::new(AtomicBool::new(true)),
            warning,
            pictures: ctx.data_dir().to_owned(),
            written: HashSet::new(),
            query: String::new(),
            shown: None,
        }
    }

    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let id = || {
            command
                .args
                .int("id")
                .and_then(|id| u64::try_from(id).ok())
                .ok_or_else(|| "no such entry".to_owned())
        };
        let result = match command.action.as_str() {
            "toggle" if self.shown.is_some() => {
                self.close(ctx);
                Ok(())
            }
            "toggle" | "open" => {
                self.open(ctx);
                Ok(())
            }
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "search" => {
                self.query = command.args.str("query").unwrap_or_default().to_owned();
                self.refresh(ctx);
                Ok(())
            }
            "pick" => id().and_then(|id| self.pick(ctx, id, self.settings.paste)),
            "copy" => id().and_then(|id| self.pick(ctx, id, false)),
            "show" => match id() {
                Ok(id) => self.show(ctx, id).await,
                Err(error) => Err(error),
            },
            "delete" => id().and_then(|id| {
                self.store.remove(id).map_err(|error| error.to_string())?;
                self.forget(id);
                self.changed(ctx);
                Ok(())
            }),
            "clear" => self
                .store
                .clear()
                .map_err(|error| error.to_string())
                .map(|()| {
                    for id in std::mem::take(&mut self.written) {
                        let _ = std::fs::remove_file(self.picture(id));
                    }
                    tracing::info!("cleared the clipboard history");
                    self.changed(ctx);
                }),
            "pause" => {
                let paused = !self.listening.load(Ordering::Relaxed);
                let paused = match command.args.str("state") {
                    Some("on") => true,
                    Some("off") => false,
                    _ => !paused,
                };
                self.listening.store(!paused, Ordering::Relaxed);
                tracing::info!(paused, "clipboard history");
                self.changed(ctx);
                Ok(())
            }
            other => Err(format!("clipboard has no action {other}")),
        };
        command.reply(result);
    }

    fn open(&mut self, ctx: &ModuleCtx) {
        // The other panels take the keyboard too; only one can be
        // open. Not awaited, as they close this the same way.
        for module in ["hub", "launcher", "audio", "tray"] {
            let close = ctx.call(module, "close", &[]);
            tokio::spawn(async move {
                match close.await {
                    Ok(()) | Err(CallError::NotEnabled(_)) => {}
                    Err(error) => tracing::warn!(%error, module, "could not close it"),
                }
            });
        }
        self.query.clear();
        let spec = ActivitySpec::new("Picker")
            .key("clipboard")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(self.payload(ctx));
        self.shown = Some(ctx.present(spec));
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown.take() {
            ctx.withdraw(id);
        }
    }

    /// Puts an entry on the clipboard, closes the picker, and pastes it.
    fn pick(&mut self, ctx: &ModuleCtx, id: u64, paste: bool) -> Result<(), String> {
        let clipboard = self
            .clipboard
            .clone()
            .ok_or("the clipboard isn't available")?;
        let formats = self.formats(id)?;
        clipboard.send(Request::Serve(formats))?;
        // Mochi's own selection isn't read back; move the entry up here.
        self.store
            .touch(id, now())
            .map_err(|error| error.to_string())?;
        self.close(ctx);
        self.publish(ctx);
        // Picked from the hub's page: the hub holds the keyboard.
        let close = ctx.call("hub", "close", &[]);
        tokio::spawn(async move {
            match close.await {
                Ok(()) | Err(CallError::NotEnabled(_)) => {}
                Err(error) => tracing::warn!(%error, "could not close the hub"),
            }
        });

        if paste {
            // While the island holds the keyboard, this is still the window
            // that had it before.
            let app = ctx.compositor().state().focused_app;
            let shift = app.as_ref().is_some_and(|app| {
                self.settings
                    .terminals
                    .iter()
                    .any(|terminal| terminal == app)
            });
            tokio::spawn(async move {
                tokio::time::sleep(REFOCUS).await;
                let _ = clipboard.send(Request::Paste { shift });
            });
        }
        Ok(())
    }

    /// Opens an image entry in the capture module's preview card, with its
    /// copy, edit and delete buttons, and closes the picker and the hub.
    async fn show(&mut self, ctx: &ModuleCtx, id: u64) -> Result<(), String> {
        let entry = self.store.get(id).ok_or("no such entry")?;
        if entry.kind != Kind::Image {
            return Err("only images open in the preview".into());
        }
        let label = if entry.width > 0 {
            format!("{} × {}", entry.width, entry.height)
        } else {
            String::from("Image")
        };
        let path = self
            .show_picture(id)
            .ok_or("cannot write the image to show it")?;
        self.close(ctx);
        let close = ctx.call("hub", "close", &[]);
        tokio::spawn(async move {
            let _ = close.await;
        });
        let path = path.display().to_string();
        let id = id.to_string();
        ctx.call("capture", "show", &[&path, &id, &label])
            .await
            .map_err(|error| match error {
                CallError::NotEnabled(_) => "the capture module isn't enabled".to_owned(),
                other => other.to_string(),
            })
    }

    /// What to serve for an entry: text under every text format, and the
    /// other formats it was copied with.
    fn formats(&mut self, id: u64) -> Result<Formats, String> {
        let kind = self.store.get(id).ok_or("no such entry")?.kind;
        let content = self.store.content(id).map_err(|error| error.to_string())?;
        let mut formats = Formats::new();
        for (index, (mime, data)) in content.into_iter().enumerate() {
            let data = Arc::new(data);
            if index == 0 && kind == Kind::Text {
                formats.extend(wayland::text_formats(data));
            } else {
                formats.push((mime, data));
            }
        }
        Ok(formats)
    }

    fn copied(&mut self, ctx: &ModuleCtx, clip: &Clip) {
        if !self.listening.load(Ordering::Relaxed) {
            return;
        }
        match self.store.add(clip, now()) {
            Ok(id) => tracing::debug!(id, "kept a copy"),
            Err(error) => {
                tracing::warn!(%error, "cannot keep a copy");
                return;
            }
        }
        self.expire(ctx);
        if let Err(error) = self.store.tidy() {
            tracing::warn!(%error, "cannot rewrite the clipboard history");
        }
        self.changed(ctx);
    }

    fn expire(&mut self, ctx: &ModuleCtx) {
        let max_age = self.settings.max_age_hours.saturating_mul(3600);
        match self.store.expire(self.settings.max_entries, max_age, now()) {
            Ok(removed) if !removed.is_empty() => {
                for id in removed {
                    self.forget(id);
                }
                self.changed(ctx);
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "cannot remove old clipboard entries"),
        }
    }

    /// Deletes an entry's picture, if one was written.
    fn forget(&mut self, id: u64) {
        if self.written.remove(&id) {
            let _ = std::fs::remove_file(self.picture(id));
        }
    }

    fn picture(&self, id: u64) -> PathBuf {
        self.pictures.join(id.to_string())
    }

    /// Writes an image entry where the picker can load it, once.
    fn show_picture(&mut self, id: u64) -> Option<PathBuf> {
        let path = self.picture(id);
        if self.written.contains(&id) {
            return Some(path);
        }
        let content = self.store.content(id).ok()?;
        let (_, data) = content.first()?;
        if let Err(error) = write_private(&path, data) {
            tracing::warn!(%error, "cannot write a clipboard image");
            return None;
        }
        self.written.insert(id);
        Some(path)
    }

    fn changed(&mut self, ctx: &ModuleCtx) {
        self.publish(ctx);
        self.refresh(ctx);
    }

    fn refresh(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown {
            let payload = self.payload(ctx);
            ctx.update(id, payload);
        }
    }

    /// The hub's state: the card's numbers and the page's entries.
    fn publish(&mut self, ctx: &ModuleCtx) {
        let entries = self.results("");
        ctx.publish_state(json!({
            "entries": entries,
            "count": self.store.entries().len(),
            "paused": !self.listening.load(Ordering::Relaxed),
            "storage": match self.storage {
                Storage::Memory => "memory",
                Storage::Disk => "disk",
            },
            "warning": self.warning,
        }));
    }

    fn payload(&mut self, ctx: &ModuleCtx) -> Value {
        let query = self.query.clone();
        let results = self.results(&query);
        json!({
            // Only the island on this monitor takes the keyboard.
            "output": ctx.compositor().state().focused_output,
            "query": self.query,
            "paused": !self.listening.load(Ordering::Relaxed),
            "now": now(),
            "results": results,
        })
    }

    /// The entries matching `query`, as the views show them.
    fn results(&mut self, query: &str) -> Vec<Value> {
        let hits: Vec<(u64, Kind)> =
            search::rank(self.store.entries(), query, self.settings.max_results)
                .iter()
                .map(|entry| (entry.id, entry.kind))
                .collect();
        let mut results = Vec::with_capacity(hits.len());
        for (id, kind) in hits {
            let picture = match kind {
                Kind::Image => self.show_picture(id),
                Kind::Text => None,
            };
            let Some(entry) = self.store.get(id) else {
                continue;
            };
            let text: String = entry.preview.chars().take(SHOWN).collect();
            results.push(json!({
                "id": id,
                "kind": match kind {
                    Kind::Text => "text",
                    Kind::Image => "image",
                },
                "text": text,
                "lines": entry.preview.lines().count(),
                "size": entry.size(),
                "time": entry.time,
                "width": entry.width,
                "height": entry.height,
                "image": picture.map(|path| format!("file://{}", path.display())),
            }));
        }
        results
    }
}

/// `$XDG_STATE_HOME/mochi/clipboard/history`, falling back to
/// `~/.local/state`.
fn disk_path() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/state")))?;
    Some(state.join("mochi").join("clipboard").join("history"))
}

/// Writes a file only the user can read.
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(data)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "clipboard",
            include_str!("../settings.toml"),
        );
    }
}

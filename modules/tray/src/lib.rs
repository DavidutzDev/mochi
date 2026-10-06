//! The tray: the icons apps like Discord, Steam or nm-applet put in a
//! system tray, through the StatusNotifierItem protocol.
//!
//! A tray bubble next to the island opens the drawer: every app's icon and
//! name on the island. A click activates the app, a right click opens its
//! menu in Mochi's style, a middle click does the app's second action.
//! Apps listed in `pinned` also get a bubble of their own, which takes the
//! same clicks and the scroll wheel. An app asking for attention makes its
//! bubble pulse.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.tray]
//! pinned = ["discord"]   # apps with a bubble of their own, by id or title
//! hidden = []            # apps never shown
//! ```
//!
//! Icons their app marks as passive, unimportant for now, stay in the
//! drawer, dimmed.

mod item;
mod menu;
mod sni;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    CallError, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use zbus::Connection;

use crate::item::{Input, Item, Status, Update};
use crate::menu::Entry;
use crate::sni::{Address, Change};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Tray;

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    pinned: Vec<String>,
    hidden: Vec<String>,
}

impl Settings {
    fn lists(list: &[String], item: &Item) -> bool {
        list.iter().any(|name| {
            name.eq_ignore_ascii_case(&item.id) || name.eq_ignore_ascii_case(&item.title)
        })
    }
}

impl Module for Tray {
    fn id(&self) -> &'static str {
        "tray"
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

    fn actions(&self) -> Vec<ActionSpec> {
        let app = || ArgSpec::string("app", "The app's key, as `list` shows it");
        vec![
            ActionSpec::new("toggle", "Open the drawer, or close it"),
            ActionSpec::new("open", "Open the drawer"),
            ActionSpec::new("close", "Close the drawer"),
            ActionSpec::new("list", "Print the apps in the tray"),
            ActionSpec::new("activate", "Do what a click on the app's icon does").arg(app()),
            ActionSpec::new("secondary", "Do what a middle click does").arg(app()),
            ActionSpec::new("scroll", "Scroll on the app's icon")
                .arg(app())
                .arg(ArgSpec::int(
                    "delta",
                    "How far; negative goes the other way",
                ))
                .arg(
                    ArgSpec::choice("orientation", "Which way", ["vertical", "horizontal"])
                        .optional(),
                ),
            ActionSpec::new("menu", "Open the app's menu on the island").arg(app()),
            ActionSpec::new("submenu", "Fill a submenu as it opens; the menu sends this")
                .arg(app())
                .arg(ArgSpec::int("entry", "The submenu's entry")),
            ActionSpec::new("click", "Click an entry of the open menu")
                .arg(app())
                .arg(ArgSpec::int("entry", "The entry")),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let (changes_sender, mut changes) = mpsc::unbounded_channel();
            let connection = sni::start(changes_sender).await?;
            let (updates_sender, mut updates) = mpsc::unbounded_channel();
            let mut state = State {
                settings,
                connection,
                icons: ctx.data_dir().to_owned(),
                updates: updates_sender,
                items: BTreeMap::new(),
                drawer: None,
                pins: HashMap::new(),
                panel: None,
                menu: None,
                attention: false,
            };
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, .. }) if state.panel == Some(activity) => {
                            state.panel = None;
                            state.menu = None;
                        }
                        Some(ModuleEvent::BubbleClicked(bubble)) => state.bubble_clicked(&ctx, bubble).await,
                        Some(_) => {}
                    },
                    Some(change) = changes.recv() => state.change(&ctx, change),
                    Some(update) = updates.recv() => state.update(&ctx, update),
                }
            }
        })
    }
}

/// An icon Mochi follows.
#[derive(Debug)]
struct Tracked {
    /// `None` until its app answered.
    item: Option<Item>,
    task: JoinHandle<()>,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The menu on the island.
#[derive(Debug)]
struct OpenMenu {
    key: String,
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct State {
    settings: Settings,
    connection: Connection,
    /// Where pictures from apps' pixels go.
    icons: PathBuf,
    updates: mpsc::UnboundedSender<Update>,
    items: BTreeMap<Address, Tracked>,
    drawer: Option<BubbleId>,
    /// Pinned apps' bubbles, by key.
    pins: HashMap<String, BubbleId>,
    panel: Option<ActivityId>,
    menu: Option<OpenMenu>,
    /// Whether an app asked for attention at the last refresh.
    attention: bool,
}

impl State {
    fn change(&mut self, ctx: &ModuleCtx, change: Change) {
        match change {
            Change::Added(address) => {
                if self.items.contains_key(&address) {
                    return;
                }
                tracing::debug!(bus = address.bus, path = address.path, "a tray icon came");
                let task = tokio::spawn(item::follow(
                    self.connection.clone(),
                    address.clone(),
                    self.icons.clone(),
                    self.updates.clone(),
                ));
                self.items.insert(address, Tracked { item: None, task });
            }
            Change::Removed(address) => {
                if self.items.remove(&address).is_some() {
                    tracing::debug!(bus = address.bus, "a tray icon went");
                    self.refresh(ctx);
                }
            }
        }
    }

    fn update(&mut self, ctx: &ModuleCtx, update: Update) {
        if let Some(tracked) = self.items.get_mut(&update.address) {
            tracked.item = Some(update.item);
            self.refresh(ctx);
        }
    }

    /// The icons shown, with the key actions name each by: its id, made
    /// unique when two apps share one. Sorted by name.
    fn shown(&self) -> Vec<(String, &Address, &Item)> {
        let mut shown: Vec<(&Address, &Item)> = self
            .items
            .iter()
            .filter_map(|(address, tracked)| Some((address, tracked.item.as_ref()?)))
            .filter(|(_, item)| !Settings::lists(&self.settings.hidden, item))
            .collect();
        shown.sort_by_key(|(address, item)| (item.name().to_lowercase(), (*address).clone()));
        let mut seen: HashMap<String, usize> = HashMap::new();
        shown
            .into_iter()
            .map(|(address, item)| {
                let base = if item.id.is_empty() {
                    item.name().to_lowercase()
                } else {
                    item.id.to_lowercase()
                };
                let count = seen.entry(base.clone()).or_default();
                *count += 1;
                let key = if *count == 1 {
                    base
                } else {
                    format!("{base}-{count}")
                };
                (key, address, item)
            })
            .collect()
    }

    fn find(&self, key: &str) -> Result<(Address, Item), String> {
        self.shown()
            .into_iter()
            .find(|(shown, _, item)| shown == key || item.name().eq_ignore_ascii_case(key))
            .map(|(_, address, item)| (address.clone(), item.clone()))
            .ok_or_else(|| format!("no app called {key} is in the tray"))
    }

    /// Updates the bubbles and the drawer after a change.
    fn refresh(&mut self, ctx: &ModuleCtx) {
        // Owned, so the bubbles can change below.
        let shown: Vec<(String, Value, bool)> = self
            .shown()
            .into_iter()
            .map(|(key, _, item)| {
                let pinned = Settings::lists(&self.settings.pinned, item);
                (key.clone(), item.to_json(&key), pinned)
            })
            .collect();
        let attention = shown.iter().any(|(_, item, _)| item["attention"] == true);

        // An app starting to ask for attention is news.
        let news = attention && !self.attention;
        self.attention = attention;
        let drawer = (!shown.is_empty()).then(|| {
            let spec = BubbleSpec::new("Drawer")
                .key("drawer")
                .area(Area::Right)
                .payload(json!({ "count": shown.len(), "attention": attention }));
            if news { spec.news() } else { spec }
        });
        match (drawer, self.drawer) {
            (Some(spec), _) => self.drawer = Some(ctx.show_bubble(spec)),
            (None, Some(bubble)) => {
                ctx.hide_bubble(bubble);
                self.drawer = None;
            }
            (None, None) => {}
        }

        let mut pins = HashMap::new();
        for (key, item, pinned) in shown {
            if !pinned {
                continue;
            }
            let spec = BubbleSpec::new("Pin")
                .key(format!("pin-{key}"))
                .area(Area::Right)
                .payload(item);
            pins.insert(key, ctx.show_bubble(spec));
        }
        for (key, bubble) in self.pins.drain() {
            if !pins.contains_key(&key) {
                ctx.hide_bubble(bubble);
            }
        }
        self.pins = pins;

        if let Some(panel) = self.panel {
            ctx.update(panel, self.payload());
        }
    }

    fn payload(&self) -> Value {
        let items: Vec<Value> = self
            .shown()
            .iter()
            .map(|(key, _, item)| item.to_json(key))
            .collect();
        let menu = self.menu.as_ref().map(|menu| {
            let title = self
                .find(&menu.key)
                .map(|(_, item)| item.name().to_owned())
                .unwrap_or_default();
            json!({
                "key": menu.key,
                "title": title,
                "entries": menu.entries.iter().map(Entry::to_json).collect::<Vec<_>>(),
            })
        });
        json!({ "items": items, "menu": menu })
    }

    async fn bubble_clicked(&mut self, ctx: &ModuleCtx, bubble: BubbleId) {
        if self.drawer == Some(bubble) {
            if self.panel.is_some() {
                self.close(ctx);
            } else {
                self.open(ctx);
            }
            return;
        }
        let pinned = self
            .pins
            .iter()
            .find(|(_, id)| **id == bubble)
            .map(|(key, _)| key.clone());
        if let Some(key) = pinned
            && let Err(error) = self.activate(ctx, &key).await
        {
            tracing::warn!(%error, "tray click");
        }
    }

    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let app = args.str("app").unwrap_or_default().to_owned();
        let entry = args
            .int("entry")
            .and_then(|entry| i32::try_from(entry).ok());
        let result = match command.action.as_str() {
            "toggle" if self.panel.is_some() => {
                self.close(ctx);
                Ok(())
            }
            "toggle" | "open" => {
                self.menu = None;
                self.open(ctx);
                Ok(())
            }
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "list" => {
                let lines: Vec<String> = self
                    .shown()
                    .iter()
                    .map(|(key, _, item)| {
                        let status = match item.status {
                            Status::Passive => "passive",
                            Status::Active => "active",
                            Status::NeedsAttention => "needs attention",
                        };
                        format!("{key}\t{}\t{status}", item.name())
                    })
                    .collect();
                command.answer(Ok(lines.join("\n")));
                return;
            }
            "activate" => self.activate(ctx, &app).await,
            "secondary" => self.input(&app, Input::Secondary).await,
            "scroll" => {
                let delta = args.int("delta").unwrap_or_default();
                let vertical = args.str("orientation") != Some("horizontal");
                let delta = i32::try_from(delta).unwrap_or_default();
                self.input(&app, Input::Scroll { delta, vertical }).await
            }
            "menu" => self.show_menu(ctx, &app).await,
            "submenu" => match entry {
                Some(entry) => self.fill_submenu(ctx, &app, entry).await,
                None => Err("which entry?".into()),
            },
            "click" => match entry {
                Some(entry) => self.click(ctx, &app, entry).await,
                None => Err("which entry?".into()),
            },
            other => Err(format!("tray has no action {other}")),
        };
        command.reply(result);
    }

    /// A click on the app's icon: what the app does with it, or its menu
    /// for apps that only have one.
    async fn activate(&mut self, ctx: &ModuleCtx, key: &str) -> Result<(), String> {
        let (address, item) = self.find(key)?;
        if item.only_menu && item.menu.is_some() {
            return self.show_menu(ctx, key).await;
        }
        match item::send(&self.connection, &address, Input::Activate).await {
            Ok(()) => {
                self.close(ctx);
                Ok(())
            }
            // Apps without Activate, like many libappindicator ones, have a
            // menu instead.
            Err(_) if item.menu.is_some() => self.show_menu(ctx, key).await,
            Err(error) => Err(format!("{}: {error}", item.name())),
        }
    }

    async fn input(&self, key: &str, input: Input) -> Result<(), String> {
        let (address, item) = self.find(key)?;
        item::send(&self.connection, &address, input)
            .await
            .map_err(|error| format!("{}: {error}", item.name()))
    }

    async fn show_menu(&mut self, ctx: &ModuleCtx, key: &str) -> Result<(), String> {
        let (address, item) = self.find(key)?;
        let path = item
            .menu
            .as_deref()
            .ok_or_else(|| format!("{} has no menu", item.name()))?;
        let entries = menu::read(&self.connection, &address.bus, path, 0)
            .await
            .map_err(|error| format!("{}'s menu: {error}", item.name()))?;
        let key = self
            .shown()
            .into_iter()
            .find(|(_, shown, _)| **shown == address)
            .map_or_else(|| key.to_owned(), |(key, ..)| key);
        self.menu = Some(OpenMenu { key, entries });
        self.open(ctx);
        Ok(())
    }

    async fn fill_submenu(&mut self, ctx: &ModuleCtx, key: &str, entry: i32) -> Result<(), String> {
        let (address, item) = self.find(key)?;
        let path = item.menu.as_deref().ok_or("no menu")?;
        let children = menu::read(&self.connection, &address.bus, path, entry)
            .await
            .map_err(|error| error.to_string())?;
        if let Some(open) = &mut self.menu
            && let Some(found) = find_entry(&mut open.entries, entry)
        {
            found.children = children;
        }
        if let Some(panel) = self.panel {
            ctx.update(panel, self.payload());
        }
        Ok(())
    }

    async fn click(&mut self, ctx: &ModuleCtx, key: &str, entry: i32) -> Result<(), String> {
        let (address, item) = self.find(key)?;
        let path = item.menu.as_deref().ok_or("no menu")?;
        menu::click(&self.connection, &address.bus, path, entry)
            .await
            .map_err(|error| format!("{}: {error}", item.name()))?;
        self.close(ctx);
        Ok(())
    }

    fn open(&mut self, ctx: &ModuleCtx) {
        // The other panels take the keyboard too; only one can be open.
        if self.panel.is_none() {
            for module in ["hub", "launcher", "clipboard", "audio", "emoji"] {
                let close = ctx.call(module, "close", &[]);
                tokio::spawn(async move {
                    match close.await {
                        Ok(()) | Err(CallError::NotEnabled(_)) => {}
                        Err(error) => tracing::warn!(%error, module, "could not close it"),
                    }
                });
            }
        }
        let spec = ActivitySpec::new("Panel")
            .key("tray")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(self.payload());
        self.panel = Some(ctx.present(spec));
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        self.menu = None;
        if let Some(panel) = self.panel.take() {
            ctx.withdraw(panel);
        }
    }
}

/// An entry anywhere in a menu, by id.
fn find_entry(entries: &mut [Entry], id: i32) -> Option<&mut Entry> {
    for entry in entries {
        if entry.id == id {
            return Some(entry);
        }
        if let Some(found) = find_entry(&mut entry.children, id) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "tray",
            include_str!("../settings.toml"),
        );
    }
}

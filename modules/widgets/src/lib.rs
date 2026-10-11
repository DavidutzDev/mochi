//! Widgets on the desktop: small views from any module, like a clock or
//! what's playing, under the windows on each monitor. Modules offer them
//! with `widgets` contributions, see [`catalog`]; the user places them, as
//! many as they like, in `widgets.toml` next to `config.toml`, see
//! [`layout`].
//!
//! `mochi ipc widgets edit` raises them over the windows on the focused
//! monitor, or the one named: drag a widget to move it, drag its corner to
//! resize it, open its settings, send it to another monitor from there, or
//! add one from the drawer, a panel at the side with every look of every
//! widget, the widgets placed, and the saved layouts, see [`saved`]. Each
//! change rewrites `widgets.toml` at once, and editing the file applies as
//! soon as it's saved.
//!
//! The module offers a clock and a calendar itself.

mod catalog;
pub mod layout;
mod lock;
mod place;
mod saved;
mod tour;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use include_dir::{Dir, include_dir};
use mochi_core::zones::{self, Offsets};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, Contribution,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::catalog::Spec;
use crate::layout::{Anchor, Layout, Placed, Rule};

pub use crate::layout::FILE;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// Checks `widgets.toml`, for `mochi config check`. `false` when there is
/// none, which is fine too.
pub fn check(path: &Path) -> Result<bool, String> {
    Layout::load(path).map(|layout| layout.is_some())
}

/// How often the file is checked for changes made by hand.
const WATCH: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub struct Widgets;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    grid: u32,
    /// Let plugins' widgets go on the lock screen too.
    lock_plugins: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            grid: 16,
            lock_plugins: false,
        }
    }
}

impl Module for Widgets {
    fn id(&self) -> &'static str {
        "widgets"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn needs(&self, _settings: &toml::Table) -> Vec<mochi_core::Need> {
        vec![mochi_core::Need::new(
            "wl-copy",
            "Copy as Nix or TOML, while the clipboard module is off",
        )]
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            // More looks go in `variants`, after the first, which placed
            // clocks from before variants have.
            ContributionSpec::new("widgets", "widget", "clock", "Clock", "Clock")
                .icon("clock")
                .options(json!({
                    "size": [14, 7],
                    "min": [8, 4],
                    "max": [40, 20],
                    "category": "Clock",
                    "variants": [
                        {
                            "id": "digital",
                            "title": "Digital",
                            "description": "The time, big, over the date",
                            "settings": ["timezone", "hours", "seconds", "date", "shape"],
                            "defaults": { "shape": "none" },
                        },
                        {
                            "id": "stacked",
                            "title": "Stacked",
                            "description": "The hour over the minutes, the date under",
                            "size": [10, 13],
                            "min": [6, 8],
                            "max": [24, 30],
                            "settings": ["timezone", "hours", "date", "shape"],
                            "defaults": { "shape": "none" },
                        },
                        {
                            "id": "analog",
                            "title": "Analog",
                            "description": "Hands and ticks, a seconds hand if you like",
                            "view": "Analog",
                            "size": [12, 12],
                            "min": [7, 7],
                            "max": [30, 30],
                            "settings": ["timezone", "seconds"],
                        },
                        {
                            "id": "shape",
                            "title": "Shape",
                            "description": "The time inside a shape in the accent color, a cookie or another",
                            "size": [11, 11],
                            "min": [7, 7],
                            "max": [24, 24],
                            "settings": ["timezone", "hours", "shape"],
                        },
                        {
                            "id": "minimal",
                            "title": "Minimal",
                            "description": "The time and the date on one line, no card",
                            "size": [20, 5],
                            "min": [12, 3],
                            "max": [50, 12],
                            "frame": false,
                            "settings": ["timezone", "hours", "date"],
                        },
                        {
                            "id": "world",
                            "title": "World",
                            "description": "The time in a few cities, and how far ahead",
                            "view": "World",
                            "size": [16, 10],
                            "min": [12, 7],
                            "max": [32, 22],
                            "settings": ["zones", "hours"],
                        },
                    ],
                    "settings": [
                        {
                            "name": "timezone",
                            "default": "",
                            "description": "A zone like Europe/Paris; empty for this computer's",
                            "source": "timezone",
                        },
                        {
                            "name": "hours",
                            "kind": "choice",
                            "choices": ["24", "12"],
                            "default": "24",
                            "description": "A 24-hour or a 12-hour clock",
                        },
                        {
                            "name": "seconds",
                            "kind": "bool",
                            "default": false,
                            "description": "Show the seconds",
                        },
                        {
                            "name": "date",
                            "kind": "bool",
                            "default": true,
                            "description": "Show the date",
                        },
                        {
                            "name": "zones",
                            "default": zones::DEFAULT.join(", "),
                            "description": "Up to four zones, with commas between",
                            "source": "timezone",
                        },
                        {
                            "name": "shape",
                            "kind": "choice",
                            "choices": [
                                "none", "cookie", "circle", "pentagon", "clover", "burst",
                                "hexagon", "octagon", "squircle", "pill",
                            ],
                            "default": "cookie",
                            "description": "The shape the time sits in, in the accent color",
                        },
                    ],
                })),
            ContributionSpec::new("widgets", "widget", "calendar", "Calendar", "Calendar")
                .icon("grid")
                .options(json!({
                    "size": [16, 15],
                    "min": [11, 10],
                    "max": [40, 38],
                    "category": "Calendar",
                    "variants": [
                        {
                            "id": "month",
                            "title": "Month",
                            "description": "This month as a grid, today marked",
                        },
                        {
                            "id": "week",
                            "title": "Week",
                            "description": "This week on a strip, today marked",
                            "size": [16, 7],
                            "min": [12, 6],
                            "max": [40, 12],
                        },
                    ],
                    "settings": [
                        {
                            "name": "first_day",
                            "kind": "choice",
                            "choices": ["monday", "sunday"],
                            "default": "monday",
                            "description": "The day weeks start on",
                        },
                    ],
                })),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let id = || ArgSpec::string("id", "The widget's id, like w1");
        let anchor = || ArgSpec::choice("anchor", "The point it's placed from", Anchor::ALL);
        vec![
            ActionSpec::new("edit", "Arrange the widgets, or stop")
                .arg(
                    ArgSpec::choice("state", "Start, stop, or flip it", ["on", "off", "toggle"])
                        .optional(),
                )
                .arg(ArgSpec::string("output", "The monitor; the focused one without").optional())
                .arg(ArgSpec::string("widget", "A widget there whose settings open").optional()),
            ActionSpec::new("edit-lock", "Arrange the lock screen's widgets, or stop")
                .arg(
                    ArgSpec::choice("state", "Start, stop, or flip it", ["on", "off", "toggle"])
                        .optional(),
                )
                .arg(ArgSpec::string("output", "The monitor; the focused one without").optional())
                .arg(ArgSpec::string("widget", "A widget there whose settings open").optional()),
            ActionSpec::new("add", "Place a widget a module offers")
                .arg(ArgSpec::string("module", "The module offering it"))
                .arg(ArgSpec::string(
                    "widget",
                    "Which of its widgets, and its look after a colon, like clock:digital",
                ))
                .arg(ArgSpec::string("output", "The monitor; the focused one without").optional())
                .arg(
                    ArgSpec::choice(
                        "anchor",
                        "The point it's placed from; the first free spot without",
                        Anchor::ALL,
                    )
                    .optional(),
                )
                .arg(ArgSpec::int("x", "Cells from the anchor").optional())
                .arg(ArgSpec::int("y", "Cells from the anchor").optional()),
            ActionSpec::new("variant", "Change a widget's look")
                .arg(id())
                .arg(ArgSpec::string(
                    "variant",
                    "One of the looks its widget offers",
                )),
            ActionSpec::new("move", "Move a widget")
                .arg(id())
                .arg(ArgSpec::string("output", "The monitor"))
                .arg(anchor())
                .arg(ArgSpec::int("x", "Cells from the anchor"))
                .arg(ArgSpec::int("y", "Cells from the anchor")),
            ActionSpec::new("resize", "Change a widget's size, in cells")
                .arg(id())
                .arg(ArgSpec::int("width", "In cells"))
                .arg(ArgSpec::int("height", "In cells")),
            ActionSpec::new("set", "Change one of a widget's settings")
                .arg(id())
                .arg(ArgSpec::string("name", "The setting"))
                .arg(ArgSpec::string("value", "Its new value").rest()),
            ActionSpec::new(
                "reset",
                "Put one of a widget's settings back to its default",
            )
            .arg(id())
            .arg(ArgSpec::string("name", "The setting")),
            ActionSpec::new("remove", "Remove a widget").arg(id()),
            ActionSpec::new(
                "drawer",
                "Open the drawer of widgets while arranging, or close it",
            )
            .arg(
                ArgSpec::choice("state", "Open, close, or flip it", ["on", "off", "toggle"])
                    .optional(),
            ),
            ActionSpec::new("layer", "Move a widget over or under the ones it overlaps")
                .arg(id())
                .arg(ArgSpec::choice(
                    "way",
                    "One layer up or down, or over or under all the others",
                    ["up", "down", "front", "back"],
                )),
            ActionSpec::new("save-layout", "Keep the arrangement under a name")
                .arg(ArgSpec::string("name", "Its name; one saved under it is replaced").rest()),
            ActionSpec::new("use-layout", "Put a saved layout in place of this one")
                .arg(ArgSpec::string("name", "The saved layout").rest()),
            ActionSpec::new(
                "show-layout",
                "Show a saved layout on a workspace, a monitor or every monitor",
            )
            .arg(ArgSpec::string("on", PLACES))
            .arg(ArgSpec::string("name", "The saved layout").rest()),
            ActionSpec::new(
                "hide-layout",
                "Stop showing a saved layout here, for what shows without it",
            )
            .arg(ArgSpec::string("on", PLACES).optional()),
            ActionSpec::new("delete-layout", "Delete a saved layout")
                .arg(ArgSpec::string("name", "The saved layout").rest()),
            ActionSpec::new("layouts", "Print the saved layouts"),
            ActionSpec::new(
                "screen",
                "Say how big a monitor is, for finding free spots; the desktop sends this",
            )
            .arg(ArgSpec::string("output", "The monitor"))
            .arg(ArgSpec::int("width", "In logical pixels"))
            .arg(ArgSpec::int("height", "In logical pixels"))
            .arg(ArgSpec::int("left", "Pixels the drawer covers at the left edge").optional())
            .arg(ArgSpec::int("right", "Pixels the drawer covers at the right edge").optional())
            .arg(ArgSpec::int("top", "Pixels the island covers at the top edge").optional())
            .arg(ArgSpec::int("bottom", "Pixels the island covers at the bottom edge").optional()),
            ActionSpec::new("export", "Print the layout, for home-manager").arg(
                ArgSpec::choice("format", "Nix, the default, or TOML", ["nix", "toml"]).optional(),
            ),
            ActionSpec::new("copy", "Copy the layout, for home-manager").arg(
                ArgSpec::choice("format", "Nix, the default, or TOML", ["nix", "toml"]).optional(),
            ),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let dir = ctx
                .config_dir()
                .map(Path::to_owned)
                .or_else(default_config_dir);
            let mut state = State::new(settings.grid, dir);
            state.lock_plugins = settings.lock_plugins;
            // Which workspace each monitor shows, for the layouts shown on
            // workspaces.
            let mut compositor = Some(ctx.compositor().subscribe());
            if let Some(updates) = &compositor {
                state.compositor_changed(&updates.borrow());
            }
            state.read_zones().await;
            state.publish(&ctx);

            let mut watch = tokio::time::interval(WATCH);
            let mut zones = tokio::time::interval(zones::REFRESH);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::Offers(offers)) => state.offered(&offers),
                        // The notice on the island opens the drawer. Where the
                        // compositor gives the click to the desktop layer
                        // instead, the layer opens it; both only open it.
                        Some(ModuleEvent::Clicked(activity)) if state.banner == Some(activity) => {
                            state.drawer = true;
                        }
                        Some(ModuleEvent::Ended { activity, .. }) if state.banner == Some(activity) => {
                            state.banner = None;
                        }
                        Some(_) => {}
                    },
                    _ = watch.tick() => {
                        if state.changed_on_disk() {
                            state.reload();
                        }
                        if state.saved_changed_on_disk() {
                            state.reload_saved();
                        }
                        if state.lock_changed_on_disk() {
                            state.reload_lock();
                        }
                    }
                    _ = zones.tick() => state.read_zones().await,
                    changed = next_change(&mut compositor) => {
                        match (changed, &mut compositor) {
                            (true, Some(updates)) => state.compositor_changed(&updates.borrow_and_update()),
                            _ => compositor = None,
                        }
                    }
                }
                if state.zones_missing() {
                    state.read_zones().await;
                }
                state.publish(&ctx);
            }
        })
    }
}

/// The first free spot on a `screen` for a widget `size` cells big: clear
/// of the `widgets` shown there, and of what the drawer and the island
/// cover.
fn free_spot(
    widgets: &[&Placed],
    screen: Screen,
    cell: f64,
    size: (u32, u32),
) -> Option<(Anchor, i32, i32)> {
    if screen.size.0 == 0 || screen.size.1 == 0 {
        return None;
    }
    let (width, height) = (f64::from(screen.size.0), f64::from(screen.size.1));
    let mut taken: Vec<place::Rect> = widgets
        .iter()
        .map(|widget| place::rect(widget, cell, (width, height)))
        .collect();
    let [left, right, top, bottom] = screen.covered.map(f64::from);
    let strip = |x, y, width, height| place::Rect {
        x,
        y,
        width,
        height,
    };
    let strips = [
        strip(0.0, 0.0, left, height),
        strip(width - right, 0.0, right, height),
        strip(0.0, 0.0, width, top),
        strip(0.0, height - bottom, width, bottom),
    ];
    taken.extend(
        strips
            .into_iter()
            .filter(|it| it.width > 0.0 && it.height > 0.0),
    );
    place::first_free(&taken, size, cell, (width, height))
}

fn no_config() -> String {
    "there is no config directory to keep widgets.toml in".into()
}

/// `$XDG_CONFIG_HOME/mochi`, for a context that doesn't say.
fn default_config_dir() -> Option<PathBuf> {
    mochi_core::config::Paths::from_env()
        .ok()
        .map(|paths| paths.config_dir)
}

/// A monitor, in logical pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Screen {
    size: (u32, u32),
    /// How far in from the left, right, top and bottom edges the drawer,
    /// while it's open, and the island cover it.
    covered: [u32; 4],
}

#[derive(Debug)]
struct State {
    /// Pixels per cell.
    grid: u32,
    path: Option<PathBuf>,
    layout: Layout,
    /// The file's modification time and size when last read or written,
    /// to tell a change by hand.
    seen: Option<(SystemTime, u64)>,
    /// Why the file couldn't be read, while it can't.
    error: Option<String>,
    /// `widget-layouts/` next to it.
    saved_dir: Option<PathBuf>,
    saved: Vec<saved::Saved>,
    /// The directory's modification time when last read.
    saved_seen: Option<SystemTime>,
    /// Why the last change to the saved layouts didn't happen.
    layout_error: Option<String>,
    specs: Vec<Spec>,
    /// Each monitor, as its desktop layer says.
    screens: BTreeMap<String, Screen>,
    /// The monitors connected, as the compositor says, and the workspace
    /// each shows, by name.
    outputs: BTreeSet<String>,
    workspaces: BTreeMap<String, String>,
    /// Every workspace of each monitor, in the compositor's order, for
    /// the drawer's Apply to.
    workspace_lists: BTreeMap<String, Vec<String>>,
    /// The monitor being arranged on.
    editing: Option<String>,
    /// The lock screen is being arranged, not the desktop.
    arranging_lock: bool,
    /// Plugins' widgets may go on the lock screen.
    lock_plugins: bool,
    /// The lock screen's layout, `lock-widgets.toml`, and when it was
    /// last read or written.
    lock: Layout,
    lock_path: Option<PathBuf>,
    lock_seen: Option<(SystemTime, u64)>,
    /// The widget whose settings open as arranging starts, like one just
    /// sent there from another monitor.
    selected: Option<String>,
    /// The notice on the island while arranging.
    banner: Option<ActivityId>,
    /// Whether the drawer is open.
    drawer: bool,
    /// The clocks' time zones, as the system knows them.
    zones: Offsets,
    /// The system's time zones for the settings' zone picker, while
    /// arranging, and when they were read.
    zone_choices: Option<(Instant, Value)>,
    published: Value,
}

impl State {
    /// With the layout in `dir`, read.
    fn new(grid: u32, dir: Option<PathBuf>) -> Self {
        let mut state = Self {
            grid: grid.max(4),
            path: dir.as_ref().map(|dir| dir.join(FILE)),
            lock_path: dir.as_ref().map(|dir| dir.join(lock::FILE)),
            lock: Layout::default(),
            lock_seen: None,
            arranging_lock: false,
            lock_plugins: false,
            saved_dir: dir.map(|dir| dir.join(saved::DIR)),
            layout: Layout::default(),
            seen: None,
            error: None,
            saved: Vec::new(),
            saved_seen: None,
            layout_error: None,
            specs: Vec::new(),
            screens: BTreeMap::new(),
            outputs: BTreeSet::new(),
            workspaces: BTreeMap::new(),
            workspace_lists: BTreeMap::new(),
            editing: None,
            selected: None,
            banner: None,
            drawer: false,
            zones: Offsets::default(),
            zone_choices: None,
            published: Value::Null,
        };
        state.reload();
        state.reload_saved();
        state.reload_lock();
        state
    }

    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let id = || args.str("id").unwrap_or_default().to_owned();
        let result = match command.action.as_str() {
            "edit" | "edit-lock" => {
                let lock = command.action == "edit-lock";
                // Switching from one to the other keeps arranging.
                let on = self.editing.is_some() && self.arranging_lock == lock;
                (self.editing, self.selected) = arranging(args, on, || focused(ctx));
                self.arranging_lock = lock && self.editing.is_some();
                // The drawer opens with it: what to add is the first thing to see.
                self.drawer = self.editing.is_some() && self.selected.is_none();
                self.announce(ctx);
                Ok(None)
            }
            "drawer" => {
                self.drawer = match args.str("state") {
                    Some("on") => true,
                    Some("off") => false,
                    _ => !self.drawer,
                } && self.editing.is_some();
                Ok(None)
            }
            "layer" => {
                let others: Vec<i32> = self
                    .source_of(&id())
                    .and_then(|source| self.layout_of(&source))
                    .map(|layout| layout.widgets.as_slice())
                    .unwrap_or_default()
                    .iter()
                    .filter(|widget| widget.id != id())
                    .map(|widget| widget.z)
                    .collect();
                let way = args.str("way").unwrap_or("up").to_owned();
                self.change(&id(), |widget| {
                    widget.z = match way.as_str() {
                        "front" => others.iter().max().map_or(0, |top| top + 1),
                        "back" => others.iter().min().map_or(0, |bottom| bottom - 1),
                        "down" => widget.z - 1,
                        _ => widget.z + 1,
                    };
                    Ok(())
                })
                .map(|()| None)
            }
            "add" => self.add(ctx, args).map(Some),
            "move" => self
                .move_widget(
                    &id(),
                    args.str("output").unwrap_or_default(),
                    args.str("anchor")
                        .and_then(Anchor::parse)
                        .unwrap_or_default(),
                    (int(args.int("x")), int(args.int("y"))),
                )
                .map(|()| None),
            "resize" => {
                let size = (cells(args.int("width")), cells(args.int("height")));
                let fitted = self
                    .placed(&id())
                    .and_then(|placed| self.look_of(placed))
                    .map_or(size, |look| look.fit(size));
                self.change(&id(), |widget| {
                    (widget.width, widget.height) = fitted;
                    Ok(())
                })
                .map(|()| None)
            }
            "variant" => {
                let wanted = args.str("variant").unwrap_or_default();
                let size = self
                    .spec_of(&id())
                    .ok_or_else(|| format!("{} isn't offered by any running module", id()))
                    .and_then(|spec| spec.variant(wanted))
                    .map(|variant| variant.size);
                size.and_then(|size| {
                    // A new look comes at its own size, from the same anchor.
                    self.change(&id(), |widget| {
                        widget.variant = Some(wanted.to_owned());
                        (widget.width, widget.height) = size;
                        Ok(())
                    })
                })
                .map(|()| None)
            }
            "screen" => {
                let pixels =
                    |name: &str| u32::try_from(args.int(name).unwrap_or_default()).unwrap_or(0);
                self.screens.insert(
                    args.str("output").unwrap_or_default().to_owned(),
                    Screen {
                        size: (pixels("width"), pixels("height")),
                        covered: ["left", "right", "top", "bottom"].map(pixels),
                    },
                );
                Ok(None)
            }
            "save-layout" => {
                let here = self.here(ctx);
                self.save_layout(args.str("name").unwrap_or_default(), &here)
                    .map(|()| None)
            }
            "show-layout" => {
                let here = self.here(ctx);
                let on = args.str("on").unwrap_or("workspace");
                self.show_layout(args.str("name").unwrap_or_default(), on, &here)
                    .map(|()| None)
            }
            "hide-layout" => {
                let here = self.here(ctx);
                self.hide_layout(args.str("on"), &here).map(|()| None)
            }
            "use-layout" => self
                .use_layout(args.str("name").unwrap_or_default())
                .map(|()| None),
            "delete-layout" => self
                .delete_layout(args.str("name").unwrap_or_default())
                .map(|gone| {
                    for widget in &gone {
                        self.forget(ctx, widget);
                    }
                    None
                }),
            "layouts" => Ok(Some(self.list_layouts())),
            "set" | "reset" => {
                let name = args.str("name").unwrap_or_default().to_owned();
                let value = match (command.action.as_str(), self.spec_of(&id())) {
                    ("reset", _) => Ok(None),
                    (_, Some(spec)) => spec
                        .setting(&name)
                        .and_then(|setting| setting.parse(args.str("value").unwrap_or_default()))
                        .map(Some),
                    (_, None) => Err(format!("{} isn't offered by any running module", id())),
                };
                value
                    .and_then(|value| {
                        self.change(&id(), |widget| {
                            match value {
                                Some(value) => widget.settings.insert(name, value),
                                None => widget.settings.remove(&name),
                            };
                            Ok(())
                        })
                    })
                    .map(|()| None)
            }
            "remove" => self.remove(ctx, &id()).map(|()| None),
            "export" => Ok(Some(self.export(args.str("format")))),
            "copy" => {
                copy(ctx, self.export(args.str("format")));
                Ok(None)
            }
            other => Err(format!("widgets has no action {other}")),
        };
        match result {
            Ok(Some(output)) => command.answer(Ok(output)),
            Ok(None) => command.reply(Ok(())),
            Err(error) => command.reply(Err(error)),
        }
    }

    fn export(&self, format: Option<&str>) -> String {
        match format {
            Some("toml") => self.layout.to_toml(),
            _ => self.layout.to_nix(),
        }
    }

    fn add(&mut self, ctx: &ModuleCtx, args: &mochi_core::Args) -> Result<String, String> {
        let module = args.str("module").unwrap_or_default();
        let (widget, variant) = match args.str("widget").unwrap_or_default().split_once(':') {
            Some((widget, variant)) => (widget, Some(variant)),
            None => (args.str("widget").unwrap_or_default(), None),
        };
        let spec = self
            .specs
            .iter()
            .find(|spec| spec.module == module && spec.widget == widget)
            .ok_or_else(|| {
                let offered: Vec<String> = self
                    .specs
                    .iter()
                    .map(|spec| format!("{} {}", spec.module, spec.widget))
                    .collect();
                format!(
                    "nothing offers {module} {widget}; offered: {}",
                    offered.join(", ")
                )
            })?;
        self.may_go(spec)?;
        if let Some(variant) = variant {
            spec.variant(variant)?;
        }
        let look = spec.look(variant);
        let (variant, size) = (look.variant.map(|variant| variant.id.clone()), look.size);
        let output = args
            .str("output")
            .filter(|output| !output.is_empty())
            .map_or_else(|| focused(ctx), str::to_owned);
        let (anchor, x, y) = match args.str("anchor").and_then(Anchor::parse) {
            Some(anchor) => (anchor, int(args.int("x")), int(args.int("y"))),
            None => self
                .free_spot(ctx, &output, size)
                .unwrap_or((Anchor::Center, 0, 0)),
        };
        let id = self
            .layout
            .new_id(self.saved.iter().map(|saved| &saved.layout));
        // Into the layout shown there.
        let source = self.source_on(&output);
        let monitor = self.monitor_in(&source, &output);
        let layout = self
            .layout_of_mut(&source)
            .ok_or_else(|| format!("no layout is saved as {}", source.name()))?;
        let top = layout.widgets.iter().map(|widget| widget.z).max();
        layout.widgets.push(Placed {
            id: id.clone(),
            module: module.to_owned(),
            widget: widget.to_owned(),
            // Kept even for the first, so it stays if the widget's looks
            // are put in another order.
            variant,
            output: monitor,
            anchor,
            x,
            y,
            width: size.0,
            height: size.1,
            // On top: later ones draw over earlier ones of the same layer.
            z: top.unwrap_or(0),
            settings: toml::Table::new(),
        });
        self.write(&source)?;
        Ok(id)
    }

    /// Whether a widget may go where it's being added: the desktop, or
    /// the lock screen while that's being arranged.
    fn may_go(&self, spec: &Spec) -> Result<(), String> {
        let (place, name) = if self.arranging_lock {
            ("lock", "lock screen")
        } else {
            ("desktop", "desktop")
        };
        if spec.goes(place, self.lock_plugins) {
            Ok(())
        } else {
            Err(format!(
                "{} {} doesn't go on the {name}",
                spec.module, spec.widget
            ))
        }
    }

    /// Whether a widget may be removed: not one its layout needs, like the
    /// lock screen's field.
    fn may_remove(&self, source: &Source, id: &str) -> Result<(), String> {
        let required = self
            .placed(id)
            .and_then(|placed| self.spec_of_placed(placed))
            .is_some_and(|spec| spec.required);
        if *source == Source::Lock && required {
            return Err("the lock screen needs its password field: move it instead".into());
        }
        Ok(())
    }

    fn remove(&mut self, ctx: &ModuleCtx, id: &str) -> Result<(), String> {
        let source = self
            .source_of(id)
            .ok_or_else(|| format!("no widget {id}"))?;
        self.may_remove(&source, id)?;
        let removed = self.take(&source, id)?;
        self.write(&source)?;
        self.forget(ctx, &removed);
        Ok(())
    }

    /// Takes a widget out of a layout, unsaved.
    fn take(&mut self, source: &Source, id: &str) -> Result<Placed, String> {
        let layout = self
            .layout_of_mut(source)
            .ok_or_else(|| format!("no widget {id}"))?;
        let index = layout
            .widgets
            .iter()
            .position(|widget| widget.id == id)
            .ok_or_else(|| format!("no widget {id}"))?;
        Ok(layout.widgets.remove(index))
    }

    /// Moves a widget on its monitor, or to another. Sent to a monitor
    /// showing another layout, it goes into that layout; one already
    /// holding a widget with its id, as layouts saved from each other do,
    /// takes the new place for that widget instead.
    fn move_widget(
        &mut self,
        id: &str,
        output: &str,
        anchor: Anchor,
        (x, y): (i32, i32),
    ) -> Result<(), String> {
        let from = self
            .source_of(id)
            .ok_or_else(|| format!("no widget {id}"))?;
        let to = self.source_on(output);
        // In the destination: on a layout shown there from another monitor,
        // it belongs to that one.
        let monitor = self.monitor_in(&to, output);
        let place = |widget: &mut Placed| {
            widget.output.clone_from(&monitor);
            widget.anchor = anchor;
            (widget.x, widget.y) = (x, y);
        };
        if from == to {
            return self.change(id, |widget| {
                place(widget);
                Ok(())
            });
        }
        let mut moved = self.take(&from, id)?;
        place(&mut moved);
        let layout = self
            .layout_of_mut(&to)
            .ok_or_else(|| format!("no layout is saved as {}", to.name()))?;
        match layout.widgets.iter_mut().find(|widget| widget.id == id) {
            Some(there) => place(there),
            None => layout.widgets.push(moved),
        }
        self.write(&from)?;
        self.write(&to)
    }

    /// Tells the module offering a widget that's gone that it can drop
    /// what it kept for it, like a note's text, unless a saved layout
    /// still has it.
    fn forget(&self, ctx: &ModuleCtx, gone: &Placed) {
        let Some(forget) = self
            .spec_of_placed(gone)
            .and_then(|spec| spec.forget.clone())
            .filter(|_| !self.kept(&gone.id))
        else {
            return;
        };
        let call = ctx.call(&gone.module, &forget, &[&gone.id]);
        let module = gone.module.clone();
        tokio::spawn(async move {
            if let Err(error) = call.await {
                tracing::warn!(%error, %module, "could not tell it a widget went");
            }
        });
    }

    /// Whether the arrangement or a saved layout has a widget with this id.
    fn kept(&self, id: &str) -> bool {
        std::iter::once(&self.layout)
            .chain(self.saved.iter().map(|saved| &saved.layout))
            .flat_map(|layout| &layout.widgets)
            .any(|widget| widget.id == id)
    }

    /// The first free spot on a monitor for a widget `size` cells big, on
    /// the size its desktop layer said, or its mode's, and clear of the
    /// drawer.
    fn free_spot(
        &self,
        ctx: &ModuleCtx,
        output: &str,
        size: (u32, u32),
    ) -> Option<(Anchor, i32, i32)> {
        let screen = self.screens.get(output).copied().or_else(|| {
            let state = ctx.compositor().state();
            let found = state.outputs.iter().find(|known| known.name == output)?;
            Some(Screen {
                size: (found.width, found.height),
                covered: [0; 4],
            })
        })?;
        free_spot(&self.shown_on(output), screen, f64::from(self.grid), size)
    }

    /// Keeps the widgets shown on `here` as `name`, and shows that layout
    /// there from then on, so a change there goes to it. Other monitors go
    /// on showing what they did, so a layout saved on each monitor is a
    /// layout for each.
    fn save_layout(&mut self, name: &str, here: &str) -> Result<(), String> {
        let result = saved::check_name(name).and_then(|name| {
            let dir = self.saved_dir.clone().ok_or_else(no_config)?;
            let widgets: Vec<Placed> = self.shown_on(here).into_iter().cloned().collect();
            let shown = self.rule_on(here).cloned();
            // Already the layout shown here, which keeps its changes: saving
            // only this monitor's part of it would drop the others'.
            if shown.as_ref().is_some_and(|rule| rule.layout == name) {
                return Ok(());
            }
            saved::save(
                &dir,
                &name,
                &Layout {
                    widgets,
                    ..Layout::default()
                },
            )?;
            self.reload_saved();
            // The entry for this monitor, or a new one for it, as close as
            // the one it replaces here.
            let rule = Rule {
                layout: name,
                output: Some(here.to_owned()),
                workspace: shown.and_then(|rule| rule.workspace),
            };
            match self
                .layout
                .show
                .iter_mut()
                .find(|other| other.same_place(&rule))
            {
                Some(other) => *other = rule,
                None => self.layout.show.push(rule),
            }
            self.save()
        });
        self.layout_done(result)
    }

    /// Shows the layout saved as `name` on a place, see [`PLACES`], in
    /// place of what showed there.
    fn show_layout(&mut self, name: &str, on: &str, here: &str) -> Result<(), String> {
        let result = saved::check_name(name).and_then(|name| {
            if !self.exists(&name) {
                return Err(format!("no layout is saved as {name}"));
            }
            let rule = self.rule_for(on, here, name)?;
            match self
                .layout
                .show
                .iter_mut()
                .find(|other| other.same_place(&rule))
            {
                Some(other) => *other = rule,
                None => self.layout.show.push(rule),
            }
            self.selected = None;
            self.save()
        });
        self.layout_done(result)
    }

    /// Stops showing the saved layout shown for a place, or the one shown
    /// on `here`, so what shows without it shows again.
    fn hide_layout(&mut self, on: Option<&str>, here: &str) -> Result<(), String> {
        let place = match on {
            Some(on) => self.rule_for(on, here, String::new()),
            None => self
                .rule_on(here)
                .cloned()
                .ok_or_else(|| "no saved layout is shown here".to_owned()),
        };
        let result = place.and_then(|place| {
            let before = self.layout.show.len();
            self.layout.show.retain(|rule| !rule.same_place(&place));
            if self.layout.show.len() == before {
                return Err(format!("no saved layout is shown on {}", place.place()));
            }
            self.selected = None;
            self.save()
        });
        self.layout_done(result)
    }

    /// A rule showing `layout` on a place, see [`PLACES`]: `workspace` and
    /// `screen` are `here`'s.
    fn rule_for(&self, on: &str, here: &str, layout: String) -> Result<Rule, String> {
        let workspace_here = || {
            self.workspaces
                .get(here)
                .cloned()
                .ok_or_else(|| format!("the compositor doesn't say which workspace {here} shows"))
        };
        let (output, workspace) = match on.trim() {
            "" => return Err("say where: a monitor, a workspace or everywhere".into()),
            "everywhere" => (None, None),
            "screen" => (Some(here.to_owned()), None),
            "workspace" => (Some(here.to_owned()), Some(workspace_here()?)),
            other => match other.split_once('/') {
                Some((_, "")) | Some(("", _)) => {
                    return Err(format!(
                        "{other:?} isn't a place: say a monitor and a workspace, like DP-3/2"
                    ));
                }
                Some(("*", workspace)) => (None, Some(workspace.to_owned())),
                Some((output, workspace)) => (Some(output.to_owned()), Some(workspace.to_owned())),
                None => (Some(other.to_owned()), None),
            },
        };
        Ok(Rule {
            layout,
            output,
            workspace,
        })
    }

    /// Puts the layout saved as `name` on the desktop. The arrangement it
    /// replaces is already saved under its own name, or is kept as
    /// "Unsaved" when it has none and isn't the same as a saved one.
    fn use_layout(&mut self, name: &str) -> Result<(), String> {
        let result = self
            .saved_dir
            .clone()
            .ok_or_else(no_config)
            .and_then(|dir| {
                let next = saved::load(&dir, name)?;
                let unsaved = self.layout.name.is_none()
                    && !self.layout.widgets.is_empty()
                    && !self
                        .saved
                        .iter()
                        .any(|saved| saved::same(&saved.layout, &self.layout));
                if unsaved {
                    saved::save(&dir, saved::UNSAVED, &self.layout)?;
                }
                self.layout = next;
                self.selected = None;
                self.save()
            });
        self.layout_done(result)
    }

    /// Deletes the layout saved as `name`, and answers its widgets, for
    /// `forget`.
    fn delete_layout(&mut self, name: &str) -> Result<Vec<Placed>, String> {
        let gone = self
            .saved
            .iter()
            .find(|saved| saved.name == name.trim())
            .map(|saved| saved.layout.widgets.clone())
            .unwrap_or_default();
        let result = self
            .saved_dir
            .clone()
            .ok_or_else(no_config)
            .and_then(|dir| {
                saved::delete(&dir, name)?;
                // The arrangement stays on the desktop, without a name.
                let named = self.layout.name.as_deref() == Some(name.trim());
                if named {
                    self.layout.name = None;
                }
                // Where it was shown, what shows without it shows again.
                let before = self.layout.show.len();
                self.layout.show.retain(|rule| rule.layout != name.trim());
                if named || self.layout.show.len() != before {
                    self.save()?;
                }
                Ok(())
            });
        self.layout_done(result).map(|()| gone)
    }

    fn list_layouts(&self) -> String {
        if self.saved.is_empty() {
            return "No saved layouts".into();
        }
        let lines: Vec<String> = self
            .saved
            .iter()
            .map(|saved| {
                let count = saved.layout.widgets.len();
                let current = self.layout.name.as_deref() == Some(saved.name.as_str());
                let places = self.places_of(&saved.name);
                format!(
                    "{}: {count} {}{}{}",
                    saved.name,
                    if count == 1 { "widget" } else { "widgets" },
                    if current { ", on the desktop" } else { "" },
                    if places.is_empty() {
                        String::new()
                    } else {
                        format!(", shown on {}", places.join(", "))
                    },
                )
            })
            .collect();
        lines.join("\n")
    }

    /// Reads the saved layouts again after a change to them, and keeps why
    /// it failed for the drawer.
    fn layout_done(&mut self, result: Result<(), String>) -> Result<(), String> {
        self.layout_error = result.as_ref().err().cloned();
        self.reload_saved();
        result
    }

    fn saved_changed_on_disk(&self) -> bool {
        let modified = self
            .saved_dir
            .as_deref()
            .and_then(|dir| std::fs::metadata(dir).ok()?.modified().ok());
        modified != self.saved_seen
    }

    fn reload_saved(&mut self) {
        let Some(dir) = &self.saved_dir else {
            return;
        };
        self.saved_seen = std::fs::metadata(dir)
            .ok()
            .and_then(|it| it.modified().ok());
        self.saved = saved::list(dir);
    }

    /// Changes one widget and saves.
    fn change(
        &mut self,
        id: &str,
        change: impl FnOnce(&mut Placed) -> Result<(), String>,
    ) -> Result<(), String> {
        let source = self
            .source_of(id)
            .ok_or_else(|| format!("no widget {id}"))?;
        let layout = self
            .layout_of_mut(&source)
            .ok_or_else(|| format!("no widget {id}"))?;
        change(layout.get_mut(id)?)?;
        self.write(&source)
    }

    /// Writes a layout that changed: `widgets.toml`, or a saved layout's
    /// file, and `widgets.toml` too when it's that saved layout.
    fn write(&mut self, source: &Source) -> Result<(), String> {
        let name = match source {
            Source::Own => return self.save(),
            Source::Lock => return self.save_lock(),
            Source::Saved(name) => name,
        };
        let dir = self.saved_dir.clone().ok_or_else(no_config)?;
        let layout = self
            .layout_of(source)
            .ok_or_else(|| format!("no layout is saved as {name}"))?
            .clone();
        saved::save(&dir, name, &layout)?;
        if self.layout.name.as_ref() == Some(name) {
            self.layout.widgets = layout.widgets;
            return self.save();
        }
        Ok(())
    }

    /// Writes `widgets.toml`, and the saved layout it is, when it has a
    /// name.
    fn save(&mut self) -> Result<(), String> {
        let path = self.path.as_ref().ok_or_else(no_config)?;
        self.layout
            .save(path)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        self.seen = stamp(path);
        self.error = None;
        if let (Some(name), Some(dir)) = (&self.layout.name, &self.saved_dir) {
            saved::save(dir, name, &self.layout)?;
            if let Some(kept) = self.saved.iter_mut().find(|saved| &saved.name == name) {
                kept.layout.widgets.clone_from(&self.layout.widgets);
            }
        }
        Ok(())
    }

    /// Writes `lock-widgets.toml`.
    fn save_lock(&mut self) -> Result<(), String> {
        let path = self.lock_path.as_ref().ok_or_else(no_config)?;
        lock::ensure_field(&mut self.lock);
        self.lock
            .save(path)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        self.lock_seen = stamp(path);
        Ok(())
    }

    fn lock_changed_on_disk(&self) -> bool {
        self.lock_path.as_deref().map(stamp) != Some(self.lock_seen)
    }

    /// Reads `lock-widgets.toml` again: the default layout without one,
    /// and the password field back when it lost it. A broken one leaves
    /// the layout as it was.
    fn reload_lock(&mut self) {
        let Some(path) = self.lock_path.clone() else {
            self.lock = lock::default_layout();
            return;
        };
        self.lock_seen = stamp(&path);
        match Layout::load(&path) {
            Ok(found) => {
                self.lock = found.unwrap_or_else(lock::default_layout);
                lock::ensure_field(&mut self.lock);
            }
            Err(error) => {
                tracing::warn!(%error, "lock-widgets.toml doesn't read; keeping the lock screen as it was");
                if self.lock.widgets.is_empty() {
                    self.lock = lock::default_layout();
                }
            }
        }
    }

    fn changed_on_disk(&self) -> bool {
        self.path.as_deref().map(stamp) != Some(self.seen)
    }

    /// Reads the file again. A broken one leaves the layout as it was.
    fn reload(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        self.seen = stamp(path);
        match Layout::load(path) {
            Ok(layout) => {
                self.layout = layout.unwrap_or_default();
                self.error = None;
            }
            Err(error) => {
                tracing::warn!(%error, "widgets.toml doesn't read; keeping the widgets as they were");
                self.error = Some(error);
            }
        }
    }

    fn offered(&mut self, offers: &[Contribution]) {
        self.specs = catalog::read(offers);
        tracing::info!(
            widgets = ?self.specs.iter().map(|spec| format!("{}/{}", spec.module, spec.widget)).collect::<Vec<_>>(),
            "widgets offered"
        );
    }

    fn placed(&self, id: &str) -> Option<&Placed> {
        self.layout_of(&self.source_of(id)?)?
            .widgets
            .iter()
            .find(|widget| widget.id == id)
    }

    fn spec_of(&self, id: &str) -> Option<&Spec> {
        self.spec_of_placed(self.placed(id)?)
    }

    fn look_of(&self, placed: &Placed) -> Option<catalog::Look<'_>> {
        Some(self.spec_of_placed(placed)?.look(placed.variant.as_deref()))
    }

    fn spec_of_placed(&self, placed: &Placed) -> Option<&Spec> {
        self.specs
            .iter()
            .find(|spec| spec.module == placed.module && spec.widget == placed.widget)
    }

    /// The time zones the clocks show: each placed clock's, and the world
    /// clock's by default, for the drawer's preview.
    fn wanted_zones(&self) -> BTreeSet<String> {
        let clock = self
            .specs
            .iter()
            .find(|spec| spec.module == "widgets" && spec.widget == "clock");
        let placed = self
            .shown()
            .into_iter()
            .map(|(widget, _)| widget)
            .filter(|widget| widget.module == "widgets" && widget.widget == "clock")
            .map(|widget| match clock {
                Some(spec) => spec.settings_for(&widget.settings, widget.variant.as_deref()),
                None => serde_json::to_value(&widget.settings).unwrap_or_default(),
            });
        let defaults = clock.map(|spec| spec.settings_for(&toml::Table::new(), Some("world")));
        placed
            .chain(defaults)
            .flat_map(|settings| clock_zones(&settings))
            .collect()
    }

    fn zones_missing(&self) -> bool {
        self.zones.missing(&self.wanted_zones())
    }

    async fn read_zones(&mut self) {
        self.zones = Offsets::read(self.wanted_zones()).await;
    }

    /// Every widget on a monitor, with that monitor: a saved layout shown
    /// on two is on both. `widgets.toml`'s own widgets for a monitor that
    /// isn't connected are there too, waiting, for the drawer's list.
    fn shown(&self) -> Vec<(&Placed, String)> {
        if self.arranging_lock {
            return self.lock_shown();
        }
        let outputs = self.known_outputs();
        let mut shown: Vec<(&Placed, String)> = self
            .layout
            .widgets
            .iter()
            .filter(|widget| {
                !outputs.contains(&widget.output) || self.source_on(&widget.output) == Source::Own
            })
            .map(|widget| (widget, widget.output.clone()))
            .collect();
        for output in &outputs {
            if let Source::Saved(_) = self.source_on(output) {
                shown.extend(
                    self.shown_on(output)
                        .into_iter()
                        .map(|widget| (widget, output.clone())),
                );
            }
        }
        shown
    }

    /// The lock screen's widgets on each monitor: those placed on it, or
    /// else those of the monitor the layout was arranged on, see
    /// [`Layout::monitor_for`].
    fn lock_shown(&self) -> Vec<(&Placed, String)> {
        let mut shown = Vec::new();
        for output in self.known_outputs() {
            let monitor = self.lock.monitor_for(&output);
            shown.extend(
                self.lock
                    .widgets
                    .iter()
                    .filter(|widget| widget.output == monitor)
                    .map(|widget| (widget, output.clone())),
            );
        }
        shown
    }

    /// A placed widget as the views get it, on `output`.
    fn widget_json(&self, widget: &Placed, output: &str) -> Value {
        let spec = self.spec_of_placed(widget);
        let look = self.look_of(widget);
        let variant = look.as_ref().and_then(|look| look.variant);
        json!({
            "id": widget.id,
            "module": widget.module,
            "widget": widget.widget,
            // The look it has, or null for a widget with one.
            "variant": variant.map(|variant| variant.id.clone()),
            "variantTitle": variant.map(|variant| variant.title.clone()),
            "output": output,
            "anchor": widget.anchor.as_str(),
            "x": widget.x,
            "y": widget.y,
            "width": widget.width,
            "height": widget.height,
            "z": widget.z,
            "settings": spec.map_or_else(
                || serde_json::to_value(&widget.settings).unwrap_or_default(),
                |spec| spec.settings_for(&widget.settings, widget.variant.as_deref()),
            ),
            // Not offered: its module isn't running.
            "view": look.as_ref().map(|look| look.view),
            // Its look's: a variant can go without the card.
            "frame": look.as_ref().is_none_or(|look| look.frame),
            "title": spec.map(|spec| spec.title.clone()),
            "icon": spec.and_then(|spec| spec.icon.clone()),
            "min": look.as_ref().map(|look| [look.min.0, look.min.1]),
            "max": look.as_ref().map(|look| [look.max.0, look.max.1]),
            // It can't be removed, like the lock screen's field.
            "required": spec.is_some_and(|spec| spec.required),
        })
    }

    fn publish(&mut self, ctx: &ModuleCtx) {
        let widgets: Vec<Value> = self
            .shown()
            .into_iter()
            .map(|(widget, output)| self.widget_json(widget, &output))
            .collect();
        // For the lock screen, whatever is being arranged.
        let lock_widgets: Vec<Value> = self
            .lock_shown()
            .into_iter()
            .map(|(widget, output)| self.widget_json(widget, &output))
            .collect();
        let layouts: Vec<Value> = self
            .saved
            .iter()
            .map(|saved| {
                json!({
                    "name": saved.name,
                    "count": saved.layout.widgets.len(),
                    // Where `[[show]]` entries show it, in words.
                    "places": self.places_of(&saved.name),
                    // And as they are, for the drawer's Apply to.
                    "rules": self
                        .layout
                        .show
                        .iter()
                        .filter(|rule| rule.layout == saved.name)
                        .map(|rule| json!({ "output": rule.output, "workspace": rule.workspace }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        // What the monitor being arranged on shows: a saved layout a
        // `[[show]]` entry shows there, and where that entry is for, or
        // `widgets.toml`'s own.
        let rule = self
            .editing
            .as_deref()
            .and_then(|here| self.rule_on(here))
            .cloned();
        let here = json!({
            "workspace": self.editing.as_ref().and_then(|here| self.workspaces.get(here)),
            "place": rule.as_ref().map(Rule::place),
            "scope": rule.as_ref().map(|rule| match (&rule.output, &rule.workspace) {
                (_, Some(_)) => "workspace",
                (Some(_), None) => "screen",
                (None, None) => "everywhere",
            }),
        });
        let current = rule.map_or_else(|| self.layout.name.clone(), |rule| Some(rule.layout));
        let (zones, unknown_zones) = self.zones.fields();
        // Only while arranging, where a clock's settings show: read again
        // after a while, for daylight saving.
        if self.editing.is_none() {
            self.zone_choices = None;
        } else if self
            .zone_choices
            .as_ref()
            .is_none_or(|(read, _)| read.elapsed() > zones::REFRESH)
        {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs() as i64);
            let choices: Value = zones::system(now).iter().map(zones::Zone::choice).collect();
            self.zone_choices = Some((Instant::now(), choices));
        }
        let state = json!({
            "grid": self.grid,
            "editing": self.editing.is_some(),
            "drawer": self.drawer,
            "output": self.editing,
            "selected": self.selected,
            "file": self.path,
            "error": self.error,
            "widgets": widgets,
            // What may be added where it's being arranged.
            "catalog": self
                .specs
                .iter()
                .filter(|spec| {
                    spec.goes(
                        if self.arranging_lock { "lock" } else { "desktop" },
                        self.lock_plugins,
                    )
                })
                .map(Spec::to_json)
                .collect::<Vec<_>>(),
            // "desktop" or "lock": what's being arranged.
            "target": if self.arranging_lock { "lock" } else { "desktop" },
            "lockWidgets": lock_widgets,
            "zones": zones,
            "unknownZones": unknown_zones,
            "timezones": self.zone_choices.as_ref().map_or(Value::Null, |(_, choices)| choices.clone()),
            // The saved layouts, and the one on the monitor being arranged
            // on, or on the desktop.
            "layouts": layouts,
            "layout": current,
            "here": here,
            // The monitors, with their workspaces, for Apply to.
            "screens": self
                .outputs
                .iter()
                .map(|output| json!({
                    "name": output,
                    "workspace": self.workspaces.get(output),
                    "workspaces": self.workspace_lists.get(output).cloned().unwrap_or_default(),
                }))
                .collect::<Vec<_>>(),
            "layoutError": self.layout_error,
        });
        if state != self.published {
            ctx.publish_state(state.clone());
            self.published = state;
        }
    }
}

/// The places `show-layout` and `hide-layout` take.
const PLACES: &str = "everywhere; screen or workspace, the one being arranged on or focused, or its workspace; a monitor, like DP-3; a workspace on it, like DP-3/2; or a workspace on any monitor, like */2";

/// Where a monitor's widgets come from: `widgets.toml`'s own, or a saved
/// layout a `[[show]]` entry shows there.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    Own,
    Saved(String),
    /// The lock screen's layout, while it's being arranged.
    Lock,
}

impl Source {
    fn name(&self) -> &str {
        match self {
            Self::Own => "",
            Self::Saved(name) => name,
            Self::Lock => "the lock screen",
        }
    }
}

impl State {
    /// Follows the monitors and the workspace each shows.
    fn compositor_changed(&mut self, state: &mochi_core::compositor::State) {
        self.outputs = state
            .outputs
            .iter()
            .map(|output| output.name.clone())
            .filter(|name| !mochi_core::compositor::is_virtual(name))
            .collect();
        self.workspaces = self
            .outputs
            .iter()
            .filter_map(|output| Some((output.clone(), state.active_on(output)?.name.clone())))
            .collect();
        self.workspace_lists = self
            .outputs
            .iter()
            .map(|output| {
                let names = state
                    .workspaces_on(output)
                    .filter(|workspace| !workspace.hidden)
                    .map(|workspace| workspace.name.clone())
                    .collect();
                (output.clone(), names)
            })
            .collect();
    }

    /// The monitor being arranged on, or the focused one.
    fn here(&self, ctx: &ModuleCtx) -> String {
        self.editing.clone().unwrap_or_else(|| focused(ctx))
    }

    fn exists(&self, name: &str) -> bool {
        self.saved.iter().any(|saved| saved.name == name)
    }

    /// The `[[show]]` entry that decides what a monitor shows now.
    fn rule_on(&self, output: &str) -> Option<&Rule> {
        let workspace = self.workspaces.get(output).map(String::as_str);
        layout::shown(&self.layout.show, output, workspace, |name| {
            self.exists(name)
        })
    }

    fn source_on(&self, output: &str) -> Source {
        if self.arranging_lock {
            return Source::Lock;
        }
        self.rule_on(output)
            .map_or(Source::Own, |rule| Source::Saved(rule.layout.clone()))
    }

    fn layout_of(&self, source: &Source) -> Option<&Layout> {
        match source {
            Source::Own => Some(&self.layout),
            Source::Lock => Some(&self.lock),
            Source::Saved(name) => self
                .saved
                .iter()
                .find(|saved| &saved.name == name)
                .map(|saved| &saved.layout),
        }
    }

    fn layout_of_mut(&mut self, source: &Source) -> Option<&mut Layout> {
        match source {
            Source::Own => Some(&mut self.layout),
            Source::Lock => Some(&mut self.lock),
            Source::Saved(name) => self
                .saved
                .iter_mut()
                .find(|saved| &saved.name == name)
                .map(|saved| &mut saved.layout),
        }
    }

    /// The widgets on a monitor: all of a saved layout shown there, or
    /// `widgets.toml`'s own placed there.
    fn shown_on(&self, output: &str) -> Vec<&Placed> {
        match self.source_on(output) {
            Source::Own => self
                .layout
                .widgets
                .iter()
                .filter(|widget| widget.output == output)
                .collect(),
            source => self.layout_of(&source).map_or_else(Vec::new, |layout| {
                let monitor = layout.monitor_for(output);
                layout
                    .widgets
                    .iter()
                    .filter(|widget| widget.output == monitor)
                    .collect()
            }),
        }
    }

    /// The monitor a widget on `output` belongs to in a layout: `output` in
    /// `widgets.toml`'s own, and in a saved layout the one whose widgets
    /// show there, see [`Layout::monitor_for`].
    fn monitor_in(&self, source: &Source, output: &str) -> String {
        match (source, self.layout_of(source)) {
            (Source::Saved(_) | Source::Lock, Some(layout)) => layout.monitor_for(output),
            _ => output.to_owned(),
        }
    }

    /// The monitors there are: the connected ones, and the ones a desktop
    /// layer spoke for.
    fn known_outputs(&self) -> BTreeSet<String> {
        self.outputs
            .iter()
            .chain(self.screens.keys())
            .cloned()
            .collect()
    }

    /// The layout a widget is changed in: the one shown on the monitor
    /// being arranged on, then one shown on another monitor, then
    /// `widgets.toml`'s own. Layouts share ids, so the same widget can be
    /// in several.
    fn source_of(&self, id: &str) -> Option<Source> {
        let has = |source: &Source| {
            self.layout_of(source)
                .is_some_and(|layout| layout.widgets.iter().any(|widget| widget.id == id))
        };
        if self.arranging_lock {
            return Some(Source::Lock).filter(has);
        }
        self.editing
            .iter()
            .chain(self.known_outputs().iter())
            .map(|output| self.source_on(output))
            .chain([Source::Own])
            .find(has)
    }

    /// Where a saved layout is shown, in words.
    fn places_of(&self, name: &str) -> Vec<String> {
        self.layout
            .show
            .iter()
            .filter(|rule| rule.layout == name)
            .map(Rule::place)
            .collect()
    }
}

/// Through the clipboard module, or `wl-copy` without it.
fn copy(ctx: &ModuleCtx, text: String) {
    let call = ctx.call("clipboard", "copy-text", &[&text]);
    tokio::spawn(async move {
        match call.await {
            Ok(()) => {}
            Err(CallError::NotEnabled(_)) => {
                let copied = mochi_core::process::spawn_detached(
                    &["wl-copy".into(), "--".into(), text],
                    None,
                );
                if let Err(error) = copied {
                    tracing::warn!(%error, "can't copy: enable the clipboard module or install wl-copy");
                }
            }
            Err(error) => tracing::warn!(%error, "the clipboard module couldn't copy it"),
        }
    });
}

impl State {
    /// Puts the notice on the island while arranging, and takes it away
    /// after. It lets clicks outside through, so they reach the widgets.
    fn announce(&mut self, ctx: &ModuleCtx) {
        match (self.editing.is_some(), self.banner) {
            (true, None) => {
                let spec = ActivitySpec::new("Editing")
                    .key("editing")
                    .priority(Priority::HIGH)
                    .uninterruptible()
                    .passive()
                    .payload(json!({}));
                self.banner = Some(ctx.present(spec));
            }
            (false, Some(banner)) => {
                ctx.withdraw(banner);
                self.banner = None;
            }
            _ => {}
        }
    }
}

/// What `edit` asks for: the monitor to arrange on, or `None` to stop, and
/// the widget whose settings open there.
fn arranging(
    args: &mochi_core::Args,
    editing: bool,
    focused: impl FnOnce() -> String,
) -> (Option<String>, Option<String>) {
    let on = match args.str("state") {
        Some("on") => true,
        Some("off") => false,
        _ => !editing,
    };
    if !on {
        return (None, None);
    }
    let output = args
        .str("output")
        .filter(|output| !output.is_empty())
        .map_or_else(focused, str::to_owned);
    (Some(output), args.str("widget").map(str::to_owned))
}

fn focused(ctx: &ModuleCtx) -> String {
    let state = ctx.compositor().state();
    state
        .focused_output
        .clone()
        .or_else(|| state.outputs.first().map(|output| output.name.clone()))
        .unwrap_or_default()
}

/// Waits for the compositor's next snapshot. `false` once no more will
/// come, and never returns while there is nothing to listen to.
async fn next_change(updates: &mut Option<mochi_core::compositor::StateReceiver>) -> bool {
    match updates {
        Some(updates) => updates.changed().await.is_ok(),
        None => std::future::pending().await,
    }
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

fn int(value: Option<i64>) -> i32 {
    value
        .unwrap_or_default()
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn cells(value: Option<i64>) -> u32 {
    u32::try_from(value.unwrap_or_default().max(1)).unwrap_or(u32::MAX)
}

/// The most zones a world clock shows.
const WORLD: usize = 4;

/// The zones a clock's settings name: its `timezone`, and a world clock's
/// `zones`, a text with commas or spaces between them, or a list in
/// `widgets.toml`. The world clock's view reads them the same way.
fn clock_zones(settings: &Value) -> Vec<String> {
    let own = settings["timezone"]
        .as_str()
        .map(str::trim)
        .filter(|zone| !zone.is_empty())
        .map(str::to_owned);
    own.into_iter()
        .chain(zones::list(&settings["zones"]).into_iter().take(WORLD))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arranges_on_the_monitor_asked_for() {
        let args = |words: &[(&str, &str)]| -> mochi_core::Args {
            words
                .iter()
                .map(|(name, value)| {
                    (
                        (*name).to_owned(),
                        mochi_core::ArgValue::String((*value).to_owned()),
                    )
                })
                .collect()
        };
        let focused = || "DP-1".to_owned();
        assert_eq!(
            arranging(&args(&[]), false, focused),
            (Some("DP-1".into()), None)
        );
        assert_eq!(arranging(&args(&[]), true, focused), (None, None));
        // Sent to another monitor: arranging follows, with its settings open.
        assert_eq!(
            arranging(
                &args(&[("state", "on"), ("output", "HDMI-A-1"), ("widget", "w2")]),
                true,
                focused
            ),
            (Some("HDMI-A-1".into()), Some("w2".into()))
        );
        assert_eq!(
            arranging(&args(&[("state", "off"), ("widget", "w2")]), true, focused),
            (None, None)
        );
    }

    fn clock(id: &str) -> Placed {
        Placed {
            id: id.into(),
            module: "widgets".into(),
            widget: "clock".into(),
            variant: None,
            output: "DP-1".into(),
            anchor: Anchor::TopLeft,
            x: 2,
            y: 2,
            width: 14,
            height: 7,
            z: 0,
            settings: toml::Table::new(),
        }
    }

    #[test]
    fn a_click_adds_clear_of_the_drawer_and_other_widgets() {
        let screen = Screen {
            size: (1600, 1000),
            covered: [0; 4],
        };
        // The widgets shown on DP-1.
        let on = |layout: &Layout| -> Vec<Placed> {
            layout
                .widgets
                .iter()
                .filter(|widget| widget.output == "DP-1")
                .cloned()
                .collect()
        };
        let spot = |widgets: Vec<Placed>, screen| {
            free_spot(&widgets.iter().collect::<Vec<_>>(), screen, 16.0, (14, 7))
        };
        let mut layout = Layout::default();
        assert_eq!(spot(on(&layout), screen), Some((Anchor::TopLeft, 2, 2)));
        // One there already; others on another monitor don't count.
        layout.widgets.push(clock("w1"));
        let mut elsewhere = clock("w2");
        elsewhere.output = "HDMI-A-1".into();
        elsewhere.y = 10;
        layout.widgets.push(elsewhere);
        assert_eq!(spot(on(&layout), screen), Some((Anchor::TopLeft, 2, 10)));
        // The drawer open on the left: past it, a cell apart, at 432 pixels,
        // which is the middle third, so anchored from the top's middle.
        let drawer = Screen {
            covered: [412, 0, 0, 0],
            ..screen
        };
        assert_eq!(spot(on(&layout), drawer), Some((Anchor::Top, -16, 2)));
        // Or on the right: the left edge is free, under the island.
        let right = Screen {
            covered: [0, 412, 52, 0],
            ..screen
        };
        assert_eq!(spot(Vec::new(), right), Some((Anchor::TopLeft, 2, 5)));
        // A monitor whose size isn't known yet.
        assert_eq!(spot(on(&layout), Screen::default()), None);
    }

    #[test]
    fn saves_switches_and_deletes_layouts() {
        let dir = std::env::temp_dir().join(format!("mochi-layouts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut state = State::new(16, Some(dir.clone()));
        let names = |state: &State| -> Vec<String> {
            state.saved.iter().map(|saved| saved.name.clone()).collect()
        };

        // Saved: what DP-1 shows is "Work", shown there from now on. Other
        // monitors go on as they were.
        state.layout.widgets.push(clock("w1"));
        let mut other = clock("w4");
        other.output = "HDMI-A-1".into();
        state.layout.widgets.push(other);
        state.save_layout(" Work ", "DP-1").unwrap();
        assert_eq!(names(&state), ["Work"]);
        let work = saved::load(&dir.join(saved::DIR), "Work").unwrap();
        assert_eq!(work.widgets, [clock("w1")]);
        assert_eq!(state.rule_on("DP-1").unwrap().layout, "Work");
        assert!(state.rule_on("HDMI-A-1").is_none());
        assert!(state.save_layout("a/b", "DP-1").is_err());
        assert!(state.layout_error.is_some());

        // A change there goes to "Work", and saving it under its own name
        // keeps it.
        state
            .move_widget("w1", "DP-1", Anchor::Center, (0, 0))
            .unwrap();
        state.save_layout("Work", "DP-1").unwrap();
        let work = saved::load(&dir.join(saved::DIR), "Work").unwrap();
        assert_eq!(work.widgets[0].anchor, Anchor::Center);

        // The file's own arrangement, named, switches with use-layout.
        state.layout.show.clear();
        state.layout.name = Some("Work".into());
        state.layout.widgets = vec![clock("w1"), clock("w2")];
        state.save().unwrap();
        assert_eq!(
            saved::load(&dir.join(saved::DIR), "Work")
                .unwrap()
                .widgets
                .len(),
            2
        );

        // An empty one, then back.
        state.layout = Layout {
            name: Some("Empty".into()),
            ..Layout::default()
        };
        state.save().unwrap();
        state.reload_saved();
        state.use_layout("Work").unwrap();
        assert_eq!(state.layout.widgets.len(), 2);
        assert_eq!(state.layout.name.as_deref(), Some("Work"));
        assert!(state.use_layout("Nothing").is_err());
        assert_eq!(state.layout.widgets.len(), 2);

        // An arrangement without a name isn't lost when another replaces it.
        state.layout = Layout {
            widgets: vec![clock("w5")],
            ..Layout::default()
        };
        state.use_layout("Empty").unwrap();
        assert_eq!(names(&state), ["Empty", "Unsaved", "Work"]);
        assert!(state.layout.widgets.is_empty());
        // New ids aren't one a saved layout has, since notes keep their text
        // by id.
        assert_eq!(
            state
                .layout
                .new_id(state.saved.iter().map(|saved| &saved.layout)),
            "w3"
        );
        assert!(state.kept("w5"));

        // Deleting the one on the desktop leaves it there, without a name.
        state.use_layout("Work").unwrap();
        let gone = state.delete_layout("Work").unwrap();
        assert_eq!(gone.len(), 2);
        assert_eq!(state.layout.name, None);
        assert_eq!(state.layout.widgets.len(), 2);
        assert_eq!(names(&state), ["Empty", "Unsaved"]);
        assert!(state.delete_layout("Work").is_err());
        let file = Layout::load(&dir.join(FILE)).unwrap().unwrap();
        assert_eq!(file.name, None);

        assert!(state.list_layouts().starts_with("Empty: 0 widgets"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn shows_saved_layouts_by_monitor_and_workspace() {
        let dir = std::env::temp_dir().join(format!("mochi-shown-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut state = State::new(16, Some(dir.clone()));
        state.outputs = ["DP-1".to_owned(), "HDMI-A-1".to_owned()].into();
        state.workspaces = [
            ("DP-1".to_owned(), "1".to_owned()),
            ("HDMI-A-1".to_owned(), "6".to_owned()),
        ]
        .into();
        let ids = |state: &State, output: &str| -> Vec<String> {
            state
                .shown_on(output)
                .iter()
                .map(|widget| widget.id.clone())
                .collect()
        };
        let saved_ids = |name: &str| -> Vec<String> {
            saved::load(&dir.join(saved::DIR), name)
                .unwrap()
                .widgets
                .into_iter()
                .map(|widget| widget.id)
                .collect()
        };

        // The file's own widget on DP-1, and a layout saved from another
        // monitor.
        state.layout.widgets.push(clock("w1"));
        state.save().unwrap();
        let mut music = clock("w7");
        music.output = "eDP-1".into();
        saved::save(
            &dir.join(saved::DIR),
            "Music",
            &Layout {
                widgets: vec![music],
                ..Layout::default()
            },
        )
        .unwrap();
        state.reload_saved();
        assert!(state.show_layout("Nothing", "screen", "DP-1").is_err());

        // On DP-1's workspace 1, all of it, whatever its widgets' monitor.
        state.show_layout("Music", "workspace", "DP-1").unwrap();
        assert_eq!(ids(&state, "DP-1"), ["w7"]);
        assert_eq!(ids(&state, "HDMI-A-1"), Vec::<String>::new());
        state.workspaces.insert("DP-1".into(), "2".into());
        assert_eq!(ids(&state, "DP-1"), ["w1"]);

        // Everywhere: on both monitors, the same widget, and the closer
        // rule still wins on workspace 1.
        state.show_layout("Music", "everywhere", "DP-1").unwrap();
        assert_eq!(ids(&state, "DP-1"), ["w7"]);
        assert_eq!(ids(&state, "HDMI-A-1"), ["w7"]);
        let shown: Vec<(String, String)> = state
            .shown()
            .into_iter()
            .map(|(widget, output)| (widget.id.clone(), output))
            .collect();
        assert!(shown.contains(&("w7".into(), "DP-1".into())));
        assert!(shown.contains(&("w7".into(), "HDMI-A-1".into())));
        assert!(!shown.iter().any(|(id, _)| id == "w1"), "{shown:?}");

        // Linked: moving it moves it in Music, so on both.
        state
            .move_widget("w7", "DP-1", Anchor::Center, (1, 1))
            .unwrap();
        let moved = saved::load(&dir.join(saved::DIR), "Music").unwrap();
        assert_eq!(
            (moved.widgets[0].anchor, moved.widgets[0].x),
            (Anchor::Center, 1)
        );

        // Work on HDMI-A-1 only: closer than everywhere.
        state.layout.name = Some("Work".into());
        state.save().unwrap();
        state.reload_saved();
        state.show_layout("Work", "screen", "HDMI-A-1").unwrap();
        assert_eq!(ids(&state, "HDMI-A-1"), ["w1"]);
        // Sent there, it goes into Work.
        state
            .move_widget("w7", "HDMI-A-1", Anchor::TopLeft, (3, 3))
            .unwrap();
        assert_eq!(saved_ids("Music"), Vec::<String>::new());
        assert_eq!(saved_ids("Work"), ["w1", "w7"]);
        // Work is the file's own arrangement too, so that follows.
        assert_eq!(state.layout.widgets.len(), 2);

        // Saved as another name where Music shows, that shows there instead,
        // and Music still shows on the other monitors.
        state.editing = Some("DP-1".into());
        state.save_layout("Calm", "DP-1").unwrap();
        assert_eq!(state.rule_on("DP-1").unwrap().layout, "Calm");
        assert_eq!(state.places_of("Calm"), ["DP-1"]);
        assert_eq!(
            state.places_of("Music"),
            ["workspace 1 on DP-1", "every screen"]
        );

        // Hiding the one for HDMI-A-1 lets the one for everywhere show.
        state.hide_layout(None, "HDMI-A-1").unwrap();
        assert_eq!(state.rule_on("HDMI-A-1").unwrap().layout, "Music");
        assert!(state.hide_layout(Some("screen"), "HDMI-A-1").is_err());

        // Deleting a layout drops where it's shown.
        state.delete_layout("Calm").unwrap();
        assert!(state.layout.show.iter().all(|rule| rule.layout != "Calm"));
        let file = Layout::load(&dir.join(FILE)).unwrap().unwrap();
        assert_eq!(file.show, state.layout.show);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_layout_across_monitors_keeps_each_widget_on_its_own() {
        let dir = std::env::temp_dir().join(format!("mochi-across-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut state = State::new(16, Some(dir.clone()));
        state.outputs = ["DP-1".to_owned(), "HDMI-A-1".to_owned()].into();
        let ids = |state: &State, output: &str| -> Vec<String> {
            state
                .shown_on(output)
                .iter()
                .map(|widget| widget.id.clone())
                .collect()
        };
        let mut right = clock("w2");
        right.output = "HDMI-A-1".into();
        let both = Layout {
            widgets: vec![clock("w1"), right],
            ..Layout::default()
        };
        saved::save(&dir.join(saved::DIR), "Both", &both).unwrap();
        saved::save(
            &dir.join(saved::DIR),
            "Left",
            &Layout {
                widgets: vec![clock("w3")],
                ..Layout::default()
            },
        )
        .unwrap();
        state.reload_saved();

        // Arranged across both: each monitor its own widgets.
        state.show_layout("Both", "everywhere", "DP-1").unwrap();
        assert_eq!(ids(&state, "DP-1"), ["w1"]);
        assert_eq!(ids(&state, "HDMI-A-1"), ["w2"]);

        // Saved from one: the same on both, and one added on the other
        // monitor joins it there, so it stays the same on both.
        state.show_layout("Left", "HDMI-A-1", "DP-1").unwrap();
        state.show_layout("Left", "DP-1", "DP-1").unwrap();
        assert_eq!(ids(&state, "DP-1"), ["w3"]);
        assert_eq!(ids(&state, "HDMI-A-1"), ["w3"]);
        assert_eq!(
            state.monitor_in(&Source::Saved("Left".into()), "HDMI-A-1"),
            "DP-1"
        );
        state
            .move_widget("w3", "HDMI-A-1", Anchor::Center, (0, 0))
            .unwrap();
        assert_eq!(ids(&state, "DP-1"), ["w3"]);

        // Places: a workspace on a monitor, or on any.
        state.show_layout("Left", "DP-1/2", "DP-1").unwrap();
        state.show_layout("Left", "*/3", "DP-1").unwrap();
        assert!(state.show_layout("Left", "DP-1/", "DP-1").is_err());
        assert!(state.show_layout("Left", "", "DP-1").is_err());
        assert_eq!(
            state.places_of("Left"),
            ["HDMI-A-1", "DP-1", "workspace 2 on DP-1", "workspace 3"]
        );
        state.hide_layout(Some("*/3"), "DP-1").unwrap();
        assert_eq!(state.places_of("Left").len(), 3);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_lock_screen_is_arranged_like_the_desktop_with_its_own_rules() {
        let dir = std::env::temp_dir().join(format!("mochi-lock-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut state = State::new(16, Some(dir.clone()));
        state.outputs = ["DP-1".to_owned(), "HDMI-A-1".to_owned()].into();
        // The default: the field, and the same on every monitor.
        assert!(
            state
                .lock
                .widgets
                .iter()
                .any(|widget| widget.widget == "field")
        );
        let field = |state: &State, output: &str| {
            state
                .lock_shown()
                .iter()
                .filter(|(widget, on)| widget.widget == "field" && on == output)
                .count()
        };
        assert_eq!((field(&state, "DP-1"), field(&state, "HDMI-A-1")), (1, 1));

        // Offers: the clock and a note may go on both, a widget that opts
        // out on the desktop only, and a plugin's on the lock screen only
        // when the setting allows.
        let offer = |module: &str, id: &str, options: Value| {
            ContributionSpec::new("widgets", "widget", id, "View", id)
                .options(options)
                .into_contribution(module)
        };
        state.offered(&[
            offer("widgets", "clock", json!({ "places": ["desktop", "lock"] })),
            offer("notes", "note", json!({})),
            offer("performance", "graphs", json!({ "places": ["desktop"] })),
            offer(
                "lock",
                "field",
                json!({ "places": ["lock"], "required": true }),
            ),
            offer("some-plugin", "spy", json!({ "plugin": true })),
        ]);
        let spec = |state: &State, id: &str| {
            state
                .specs
                .iter()
                .find(|spec| spec.widget == id)
                .unwrap()
                .clone()
        };
        assert!(state.may_go(&spec(&state, "clock")).is_ok());
        assert!(
            state.may_go(&spec(&state, "field")).is_err(),
            "not on the desktop"
        );
        state.arranging_lock = true;
        state.editing = Some("DP-1".into());
        assert!(state.may_go(&spec(&state, "clock")).is_ok());
        assert!(state.may_go(&spec(&state, "note")).is_ok());
        assert!(state.may_go(&spec(&state, "graphs")).is_err());
        assert!(state.may_go(&spec(&state, "spy")).is_err());
        state.lock_plugins = true;
        assert!(state.may_go(&spec(&state, "spy")).is_ok());
        state.lock_plugins = false;

        // The field stays; moving it goes to lock-widgets.toml, on both.
        let id = state
            .lock
            .widgets
            .iter()
            .find(|widget| widget.widget == "field")
            .unwrap()
            .id
            .clone();
        assert_eq!(state.source_of(&id), Some(Source::Lock));
        assert!(state.may_remove(&Source::Lock, &id).is_err());
        state
            .move_widget(&id, "DP-1", Anchor::Center, (0, 10))
            .unwrap();
        let saved = Layout::load(&dir.join(lock::FILE)).unwrap().unwrap();
        let moved = saved.widgets.iter().find(|widget| widget.id == id).unwrap();
        assert_eq!((moved.anchor, moved.y), (Anchor::Center, 10));
        assert_eq!(field(&state, "HDMI-A-1"), 1);
        // The desktop's file didn't change.
        assert!(state.layout.widgets.is_empty());

        // Without a field, the file gets it back on reading.
        let mut broken = saved.clone();
        broken.widgets.retain(|widget| widget.widget != "field");
        broken.save(&dir.join(lock::FILE)).unwrap();
        state.reload_lock();
        assert!(
            state
                .lock
                .widgets
                .iter()
                .any(|widget| widget.widget == "field")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_clock_offer_reads() {
        let offers: Vec<Contribution> = Widgets
            .contributions()
            .into_iter()
            .map(|spec| spec.into_contribution("widgets"))
            .collect();
        let specs = catalog::read(&offers);
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].settings.len(), 6);
        // The digital look stays first, and the minimal one has no card.
        assert_eq!(specs[0].look(None).variant.unwrap().id, "digital");
        // The shape look starts in a cookie, the digital one in no shape.
        let none = toml::Table::new();
        assert_eq!(
            specs[0].settings_for(&none, Some("shape"))["shape"],
            "cookie"
        );
        assert_eq!(specs[0].settings_for(&none, None)["shape"], "none");
        assert!(!specs[0].look(Some("minimal")).frame);
        assert_eq!(specs[0].look(Some("world")).view, "World");
        assert_eq!(specs[1].look(None).variant.unwrap().id, "month");
    }

    #[test]
    fn reads_the_zones_a_clock_names() {
        assert_eq!(
            clock_zones(&json!({ "timezone": "Asia/Tokyo", "zones": "" })),
            ["Asia/Tokyo"]
        );
        assert_eq!(
            clock_zones(&json!({
                "timezone": "",
                "zones": " Europe/London,America/New_York  Asia/Kolkata, ",
            })),
            ["Europe/London", "America/New_York", "Asia/Kolkata"]
        );
        // A list, written in widgets.toml by hand, and no more than four.
        assert_eq!(
            clock_zones(&json!({ "zones": ["UTC", "A/B", "C/D", "E/F", "G/H"] })),
            ["UTC", "A/B", "C/D", "E/F"]
        );
        assert!(clock_zones(&json!({})).is_empty());
    }

    #[test]
    fn reads_the_zones_of_placed_clocks_and_the_preview() {
        let dir = std::env::temp_dir().join(format!("mochi-zones-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut state = State::new(16, Some(dir.clone()));
        // Before the clock is offered, only what's placed.
        let mut tokyo = clock("w1");
        tokyo
            .settings
            .insert("timezone".into(), toml::Value::String("Asia/Tokyo".into()));
        state.layout.widgets.push(tokyo);
        assert_eq!(state.wanted_zones(), BTreeSet::from(["Asia/Tokyo".into()]));

        // Offered: the world clock's default zones too, for its preview.
        let offers: Vec<Contribution> = Widgets
            .contributions()
            .into_iter()
            .map(|spec| spec.into_contribution("widgets"))
            .collect();
        state.offered(&offers);
        let mut world = clock("w2");
        world.variant = Some("world".into());
        world
            .settings
            .insert("zones".into(), toml::Value::String("Europe/Paris".into()));
        state.layout.widgets.push(world);
        let wanted = state.wanted_zones();
        for zone in [
            "Asia/Tokyo",
            "Europe/Paris",
            "Europe/London",
            "America/New_York",
        ] {
            assert!(wanted.contains(zone), "{zone} in {wanted:?}");
        }

        // Read: an unknown zone isn't asked again every turn.
        state.zones.known = wanted
            .iter()
            .filter(|zone| *zone != "Europe/Paris")
            .map(|zone| (zone.clone(), 0))
            .collect();
        assert!(state.zones_missing());
        state.zones.unknown.insert("Europe/Paris".into());
        assert!(!state.zones_missing());
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "widgets",
            include_str!("../settings.toml"),
        );
    }
}

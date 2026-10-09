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
mod place;
mod saved;
mod tour;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::zones::{self, Offsets};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, Contribution,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::catalog::Spec;
use crate::layout::{Anchor, Layout, Placed};

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
}

impl Default for Settings {
    fn default() -> Self {
        Self { grid: 16 }
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
                            "settings": ["timezone", "hours", "seconds", "date"],
                        },
                        {
                            "id": "stacked",
                            "title": "Stacked",
                            "description": "The hour over the minutes, the date under",
                            "size": [10, 13],
                            "min": [6, 8],
                            "max": [24, 30],
                            "settings": ["timezone", "hours", "date"],
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
                            "description": "The time inside a cookie in the accent color",
                            "size": [11, 11],
                            "min": [7, 7],
                            "max": [24, 24],
                            "settings": ["timezone", "hours"],
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
                    }
                    _ = zones.tick() => state.read_zones().await,
                }
                if state.zones_missing() {
                    state.read_zones().await;
                }
                state.publish(&ctx);
            }
        })
    }
}

/// The first free spot on `output`, a `screen`, for a widget `size` cells
/// big: clear of the widgets there, and of what the drawer and the island
/// cover.
fn free_spot(
    layout: &Layout,
    output: &str,
    screen: Screen,
    cell: f64,
    size: (u32, u32),
) -> Option<(Anchor, i32, i32)> {
    if screen.size.0 == 0 || screen.size.1 == 0 {
        return None;
    }
    let (width, height) = (f64::from(screen.size.0), f64::from(screen.size.1));
    let mut taken: Vec<place::Rect> = layout
        .widgets
        .iter()
        .filter(|widget| widget.output == output)
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
    /// The monitor being arranged on.
    editing: Option<String>,
    /// The widget whose settings open as arranging starts, like one just
    /// sent there from another monitor.
    selected: Option<String>,
    /// The notice on the island while arranging.
    banner: Option<ActivityId>,
    /// Whether the drawer is open.
    drawer: bool,
    /// The clocks' time zones, as the system knows them.
    zones: Offsets,
    published: Value,
}

impl State {
    /// With the layout in `dir`, read.
    fn new(grid: u32, dir: Option<PathBuf>) -> Self {
        let mut state = Self {
            grid: grid.max(4),
            path: dir.as_ref().map(|dir| dir.join(FILE)),
            saved_dir: dir.map(|dir| dir.join(saved::DIR)),
            layout: Layout::default(),
            seen: None,
            error: None,
            saved: Vec::new(),
            saved_seen: None,
            layout_error: None,
            specs: Vec::new(),
            screens: BTreeMap::new(),
            editing: None,
            selected: None,
            banner: None,
            drawer: false,
            zones: Offsets::default(),
            published: Value::Null,
        };
        state.reload();
        state.reload_saved();
        state
    }

    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let id = || args.str("id").unwrap_or_default().to_owned();
        let result = match command.action.as_str() {
            "edit" => {
                (self.editing, self.selected) =
                    arranging(args, self.editing.is_some(), || focused(ctx));
                self.drawer = false;
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
                    .layout
                    .widgets
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
                .change(&id(), |widget| {
                    widget.output = args.str("output").unwrap_or_default().to_owned();
                    widget.anchor = args
                        .str("anchor")
                        .and_then(Anchor::parse)
                        .unwrap_or_default();
                    widget.x = int(args.int("x"));
                    widget.y = int(args.int("y"));
                    Ok(())
                })
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
            "save-layout" => self
                .save_layout(args.str("name").unwrap_or_default())
                .map(|()| None),
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
        self.layout.widgets.push(Placed {
            id: id.clone(),
            module: module.to_owned(),
            widget: widget.to_owned(),
            // Kept even for the first, so it stays if the widget's looks
            // are put in another order.
            variant,
            output,
            anchor,
            x,
            y,
            width: size.0,
            height: size.1,
            // On top: later ones draw over earlier ones of the same layer.
            z: self
                .layout
                .widgets
                .iter()
                .map(|widget| widget.z)
                .max()
                .unwrap_or(0),
            settings: toml::Table::new(),
        });
        self.save()?;
        Ok(id)
    }

    fn remove(&mut self, ctx: &ModuleCtx, id: &str) -> Result<(), String> {
        let index = self
            .layout
            .widgets
            .iter()
            .position(|widget| widget.id == id)
            .ok_or_else(|| format!("no widget {id}"))?;
        let removed = self.layout.widgets.remove(index);
        self.save()?;
        self.forget(ctx, &removed);
        Ok(())
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
        free_spot(&self.layout, output, screen, f64::from(self.grid), size)
    }

    /// Keeps the arrangement as `name`, which it then is.
    fn save_layout(&mut self, name: &str) -> Result<(), String> {
        let result = saved::check_name(name).and_then(|name| {
            self.layout.name = Some(name);
            self.save()
        });
        self.layout_done(result)
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
                if self.layout.name.as_deref() == Some(name.trim()) {
                    self.layout.name = None;
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
                format!(
                    "{}: {count} {}{}",
                    saved.name,
                    if count == 1 { "widget" } else { "widgets" },
                    if current { ", on the desktop" } else { "" },
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
        change(self.layout.get_mut(id)?)?;
        self.save()
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
        self.layout.widgets.iter().find(|widget| widget.id == id)
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
            .layout
            .widgets
            .iter()
            .filter(|widget| widget.module == "widgets" && widget.widget == "clock")
            .map(|widget| match clock {
                Some(spec) => spec.settings_for(&widget.settings),
                None => serde_json::to_value(&widget.settings).unwrap_or_default(),
            });
        let defaults = clock.map(|spec| spec.settings_for(&toml::Table::new()));
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

    fn publish(&mut self, ctx: &ModuleCtx) {
        let widgets: Vec<Value> = self
            .layout
            .widgets
            .iter()
            .map(|widget| {
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
                    "output": widget.output,
                    "anchor": widget.anchor.as_str(),
                    "x": widget.x,
                    "y": widget.y,
                    "width": widget.width,
                    "height": widget.height,
                    "z": widget.z,
                    "settings": spec.map_or_else(
                        || serde_json::to_value(&widget.settings).unwrap_or_default(),
                        |spec| spec.settings_for(&widget.settings),
                    ),
                    // Not offered: its module isn't running.
                    "view": look.as_ref().map(|look| look.view),
                    // Its look's: a variant can go without the card.
                    "frame": look.as_ref().is_none_or(|look| look.frame),
                    "title": spec.map(|spec| spec.title.clone()),
                    "icon": spec.and_then(|spec| spec.icon.clone()),
                    "min": look.as_ref().map(|look| [look.min.0, look.min.1]),
                    "max": look.as_ref().map(|look| [look.max.0, look.max.1]),
                })
            })
            .collect();
        let layouts: Vec<Value> = self
            .saved
            .iter()
            .map(|saved| {
                json!({
                    "name": saved.name,
                    "count": saved.layout.widgets.len(),
                })
            })
            .collect();
        let (zones, unknown_zones) = self.zones.fields();
        let state = json!({
            "grid": self.grid,
            "editing": self.editing.is_some(),
            "drawer": self.drawer,
            "output": self.editing,
            "selected": self.selected,
            "file": self.path,
            "error": self.error,
            "widgets": widgets,
            "catalog": self.specs.iter().map(Spec::to_json).collect::<Vec<_>>(),
            "zones": zones,
            "unknownZones": unknown_zones,
            // The saved layouts, and the one on the desktop.
            "layouts": layouts,
            "layout": self.layout.name,
            "layoutError": self.layout_error,
        });
        if state != self.published {
            ctx.publish_state(state.clone());
            self.published = state;
        }
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
        let mut layout = Layout::default();
        assert_eq!(
            free_spot(&layout, "DP-1", screen, 16.0, (14, 7)),
            Some((Anchor::TopLeft, 2, 2))
        );
        // One there already; others on another monitor don't count.
        layout.widgets.push(clock("w1"));
        let mut elsewhere = clock("w2");
        elsewhere.output = "HDMI-A-1".into();
        elsewhere.y = 10;
        layout.widgets.push(elsewhere);
        assert_eq!(
            free_spot(&layout, "DP-1", screen, 16.0, (14, 7)),
            Some((Anchor::TopLeft, 2, 10))
        );
        // The drawer open on the left: past it, a cell apart, at 432 pixels,
        // which is the middle third, so anchored from the top's middle.
        let drawer = Screen {
            covered: [412, 0, 0, 0],
            ..screen
        };
        assert_eq!(
            free_spot(&layout, "DP-1", drawer, 16.0, (14, 7)),
            Some((Anchor::Top, -16, 2))
        );
        // Or on the right: the left edge is free, under the island.
        let right = Screen {
            covered: [0, 412, 52, 0],
            ..screen
        };
        assert_eq!(
            free_spot(&Layout::default(), "DP-1", right, 16.0, (14, 7)),
            Some((Anchor::TopLeft, 2, 5))
        );
        // A monitor whose size isn't known yet.
        assert_eq!(
            free_spot(&layout, "DP-1", Screen::default(), 16.0, (14, 7)),
            None
        );
    }

    #[test]
    fn saves_switches_and_deletes_layouts() {
        let dir = std::env::temp_dir().join(format!("mochi-layouts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut state = State::new(16, Some(dir.clone()));
        let names = |state: &State| -> Vec<String> {
            state.saved.iter().map(|saved| saved.name.clone()).collect()
        };

        // Saved: the arrangement is "Work" from now on.
        state.layout.widgets.push(clock("w1"));
        state.save_layout(" Work ").unwrap();
        assert_eq!(state.layout.name.as_deref(), Some("Work"));
        assert_eq!(names(&state), ["Work"]);
        assert!(state.save_layout("a/b").is_err());
        assert!(state.layout_error.is_some());

        // A change to it goes to "Work" too.
        state.layout.widgets.push(clock("w2"));
        state.save().unwrap();
        assert_eq!(
            saved::load(&dir.join(saved::DIR), "Work")
                .unwrap()
                .widgets
                .len(),
            2
        );

        // An empty one, then back.
        state.layout = Layout::default();
        state.save_layout("Empty").unwrap();
        state.use_layout("Work").unwrap();
        assert_eq!(state.layout.widgets.len(), 2);
        assert_eq!(state.layout.name.as_deref(), Some("Work"));
        assert!(state.use_layout("Nothing").is_err());
        assert_eq!(state.layout.widgets.len(), 2);

        // An arrangement without a name isn't lost when another replaces it.
        state.layout = Layout {
            name: None,
            widgets: vec![clock("w5")],
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
    fn the_clock_offer_reads() {
        let offers: Vec<Contribution> = Widgets
            .contributions()
            .into_iter()
            .map(|spec| spec.into_contribution("widgets"))
            .collect();
        let specs = catalog::read(&offers);
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].settings.len(), 5);
        // The digital look stays first, and the minimal one has no card.
        assert_eq!(specs[0].look(None).variant.unwrap().id, "digital");
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

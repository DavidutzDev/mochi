//! Widgets on the desktop: small views from any module, like a clock or
//! what's playing, under the windows on each monitor. Modules offer them
//! with `widgets` contributions, see [`catalog`]; the user places them, as
//! many as they like, in `widgets.toml` next to `config.toml`, see
//! [`layout`].
//!
//! `mochi ipc widgets edit` raises them over the windows on the focused
//! monitor: drag a widget to move it, drag its corner to resize it, open
//! its settings, or drag a new one from the drawer. Each change rewrites
//! `widgets.toml` at once, and editing the file applies as soon as it's
//! saved.
//!
//! The module offers a clock itself.

mod catalog;
mod layout;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ArgSpec, Assets, BoxFuture, CallError, Contribution, ContributionSpec, Module,
    ModuleCommand, ModuleCtx, ModuleError, ModuleEvent,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;

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
/// How often the clocks' time zones are read again, for daylight saving.
const ZONES: Duration = Duration::from_secs(600);

#[derive(Debug, Default)]
pub struct Widgets;

#[derive(Debug, Deserialize, PartialEq)]
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

    fn check_settings(&self, table: &toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("widgets", "widget", "clock", "Clock", "Clock")
                .icon("clock")
                .options(json!({
                    "size": [14, 7],
                    "min": [8, 4],
                    "max": [40, 20],
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
                            "description": "Show the date under the time",
                        },
                    ],
                })),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let id = || ArgSpec::string("id", "The widget's id, like w1");
        let anchor = || ArgSpec::choice("anchor", "The point it's placed from", Anchor::ALL);
        vec![
            ActionSpec::new("edit", "Arrange the widgets, or stop").arg(
                ArgSpec::choice("state", "Start, stop, or flip it", ["on", "off", "toggle"])
                    .optional(),
            ),
            ActionSpec::new("add", "Place a widget a module offers")
                .arg(ArgSpec::string("module", "The module offering it"))
                .arg(ArgSpec::string("widget", "Which of its widgets"))
                .arg(ArgSpec::string("output", "The monitor; the focused one without").optional())
                .arg(anchor().optional())
                .arg(ArgSpec::int("x", "Cells from the anchor").optional())
                .arg(ArgSpec::int("y", "Cells from the anchor").optional()),
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
            let mut state = State {
                grid: settings.grid.max(4),
                path: dir.map(|dir| dir.join(FILE)),
                layout: Layout::default(),
                seen: None,
                error: None,
                specs: Vec::new(),
                editing: None,
                zones: BTreeMap::new(),
                published: Value::Null,
            };
            state.reload();
            state.read_zones().await;
            state.publish(&ctx);

            let mut watch = tokio::time::interval(WATCH);
            let mut zones = tokio::time::interval(ZONES);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::Offers(offers)) => state.offered(&offers),
                        Some(_) => {}
                    },
                    _ = watch.tick() => {
                        if state.changed_on_disk() {
                            state.reload();
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

/// `$XDG_CONFIG_HOME/mochi`, for a context that doesn't say.
fn default_config_dir() -> Option<PathBuf> {
    mochi_core::config::Paths::from_env()
        .ok()
        .map(|paths| paths.config_dir)
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
    specs: Vec<Spec>,
    /// The monitor being arranged on.
    editing: Option<String>,
    /// UTC offsets in seconds, by time zone, for the clocks.
    zones: BTreeMap<String, i64>,
    published: Value,
}

impl State {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let id = || args.str("id").unwrap_or_default().to_owned();
        let result = match command.action.as_str() {
            "edit" => {
                let on = match args.str("state") {
                    Some("on") => true,
                    Some("off") => false,
                    _ => self.editing.is_none(),
                };
                self.editing = on.then(|| focused(ctx));
                Ok(None)
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
                let spec = self.spec_of(&id()).cloned();
                self.change(&id(), |widget| {
                    let size = (cells(args.int("width")), cells(args.int("height")));
                    (widget.width, widget.height) = match &spec {
                        Some(spec) => spec.fit(size),
                        None => size,
                    };
                    Ok(())
                })
                .map(|()| None)
            }
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
        let widget = args.str("widget").unwrap_or_default();
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
        let id = self.layout.new_id();
        self.layout.widgets.push(Placed {
            id: id.clone(),
            module: module.to_owned(),
            widget: widget.to_owned(),
            output: args
                .str("output")
                .filter(|output| !output.is_empty())
                .map_or_else(|| focused(ctx), str::to_owned),
            anchor: args
                .str("anchor")
                .and_then(Anchor::parse)
                .unwrap_or(Anchor::Center),
            x: int(args.int("x")),
            y: int(args.int("y")),
            width: spec.size.0,
            height: spec.size.1,
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
        // The module may keep things for it, like a note's text.
        if let Some(forget) = self
            .spec_of_placed(&removed)
            .and_then(|spec| spec.forget.clone())
        {
            let call = ctx.call(&removed.module, &forget, &[id]);
            let module = removed.module.clone();
            tokio::spawn(async move {
                if let Err(error) = call.await {
                    tracing::warn!(%error, %module, "could not tell it a widget went");
                }
            });
        }
        Ok(())
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

    fn save(&mut self) -> Result<(), String> {
        let path = self
            .path
            .as_ref()
            .ok_or("there is no config directory to keep widgets.toml in")?;
        self.layout
            .save(path)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        self.seen = stamp(path);
        self.error = None;
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

    fn spec_of(&self, id: &str) -> Option<&Spec> {
        let placed = self.layout.widgets.iter().find(|widget| widget.id == id)?;
        self.spec_of_placed(placed)
    }

    fn spec_of_placed(&self, placed: &Placed) -> Option<&Spec> {
        self.specs
            .iter()
            .find(|spec| spec.module == placed.module && spec.widget == placed.widget)
    }

    /// The time zones the clocks show.
    fn wanted_zones(&self) -> BTreeSet<String> {
        self.layout
            .widgets
            .iter()
            .filter(|widget| widget.module == "widgets" && widget.widget == "clock")
            .filter_map(|widget| widget.settings.get("timezone")?.as_str())
            .filter(|zone| !zone.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn zones_missing(&self) -> bool {
        self.wanted_zones()
            .iter()
            .any(|zone| !self.zones.contains_key(zone))
    }

    async fn read_zones(&mut self) {
        let mut zones = BTreeMap::new();
        for zone in self.wanted_zones() {
            match utc_offset(&zone).await {
                Some(offset) => {
                    zones.insert(zone, offset);
                }
                None => tracing::warn!(%zone, "unknown time zone"),
            }
        }
        // Unknown ones aren't asked again every turn.
        for zone in self.wanted_zones() {
            zones.entry(zone).or_insert(0);
        }
        self.zones = zones;
    }

    fn publish(&mut self, ctx: &ModuleCtx) {
        let widgets: Vec<Value> = self
            .layout
            .widgets
            .iter()
            .map(|widget| {
                let spec = self.spec_of_placed(widget);
                json!({
                    "id": widget.id,
                    "module": widget.module,
                    "widget": widget.widget,
                    "output": widget.output,
                    "anchor": widget.anchor.as_str(),
                    "x": widget.x,
                    "y": widget.y,
                    "width": widget.width,
                    "height": widget.height,
                    "settings": spec.map_or_else(
                        || serde_json::to_value(&widget.settings).unwrap_or_default(),
                        |spec| spec.settings_for(&widget.settings),
                    ),
                    // Not offered: its module isn't running.
                    "view": spec.map(|spec| spec.view.clone()),
                    "frame": spec.is_none_or(|spec| spec.frame),
                    "title": spec.map(|spec| spec.title.clone()),
                    "min": spec.map(|spec| [spec.min.0, spec.min.1]),
                    "max": spec.map(|spec| [spec.max.0, spec.max.1]),
                })
            })
            .collect();
        let state = json!({
            "grid": self.grid,
            "editing": self.editing.is_some(),
            "output": self.editing,
            "file": self.path,
            "error": self.error,
            "widgets": widgets,
            "catalog": self.specs.iter().map(Spec::to_json).collect::<Vec<_>>(),
            "zones": self.zones,
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

/// A zone's offset from UTC now, in seconds, from `date`. `None` for a
/// zone the system doesn't have, which `date` would quietly take as UTC.
async fn utc_offset(zone: &str) -> Option<i64> {
    let known = zone_dirs().any(|dir| dir.join(zone).is_file()) && !zone.contains("..");
    if !known {
        return None;
    }
    let output = Command::new("date")
        .arg("+%z")
        .env("TZ", zone)
        .output()
        .await
        .ok()?;
    parse_offset(String::from_utf8(output.stdout).ok()?.trim())
}

/// Where the system keeps its time zones: `TZDIR`, or the usual places.
fn zone_dirs() -> impl Iterator<Item = PathBuf> {
    std::env::var_os("TZDIR")
        .map(PathBuf::from)
        .into_iter()
        .chain(["/usr/share/zoneinfo", "/etc/zoneinfo"].map(PathBuf::from))
}

/// `+0200` or `-0330` as seconds.
fn parse_offset(text: &str) -> Option<i64> {
    let (sign, digits) = match text.as_bytes().first()? {
        b'+' => (1, &text[1..]),
        b'-' => (-1, &text[1..]),
        _ => return None,
    };
    if digits.len() != 4 {
        return None;
    }
    let hours: i64 = digits[..2].parse().ok()?;
    let minutes: i64 = digits[2..].parse().ok()?;
    Some(sign * (hours * 3600 + minutes * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_utc_offsets() {
        assert_eq!(parse_offset("+0200"), Some(7200));
        assert_eq!(parse_offset("-0330"), Some(-12600));
        assert_eq!(parse_offset("0200"), None);
        assert_eq!(parse_offset("+02"), None);
    }

    #[test]
    fn the_clock_offer_reads() {
        let offers: Vec<Contribution> = Widgets
            .contributions()
            .into_iter()
            .map(|spec| spec.into_contribution("widgets"))
            .collect();
        let specs = catalog::read(&offers);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].settings.len(), 4);
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

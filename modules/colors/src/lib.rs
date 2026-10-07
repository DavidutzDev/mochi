//! A color picker. `mochi ipc colors pick` freezes every screen and shows
//! a magnifier at the pointer; a click picks the pixel under it. The color
//! is copied in the default format, kept in the history, and shown on the
//! island with a row for each format, which copies it.
//!
//! The history is kept in `$XDG_STATE_HOME/mochi/colors.json`, newest
//! first. The hub gets a card with the latest colors and a page with all
//! of them, and the launcher a `#` provider: `#` alone offers to pick and
//! lists the history, `#` with a color lists it in every format.
//!
//! The overlay shows the frozen screens, but the pixels come from copies
//! the module makes as the picker opens, through screencopy: see
//! [`screen`]. The overlay sends where the pointer is, in the screen's own
//! pixels, and gets the pixels around it back for its magnifier. The math,
//! from parsing to each format, is in [`color`], so views get finished
//! text.
//!
//! Settings in `config.toml`: see `settings.toml`.

mod color;
mod history;
mod screen;

use std::path::PathBuf;
use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Args, Assets, BoxFuture, CallError,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::time::Instant;

use crate::color::{Color, Format};
use crate::history::History;
use crate::screen::Frame;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long the hub, the launcher or the last color's card takes to go
/// before the screen freezes, when the picker opens over one of them.
const CLEARS: Duration = Duration::from_millis(350);
/// How long the island shows a picked color.
const CARD: Duration = Duration::from_secs(8);
/// The most named colors the launcher lists for a word that isn't a color.
const NAMES_LISTED: usize = 8;
/// How many pixels across the magnifier shows; odd, so one is in the
/// middle.
const LENS: i64 = 11;

#[derive(Debug, Default)]
pub struct Colors;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    format: Format,
    history: usize,
    uppercase: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: Format::Hex,
            history: 50,
            uppercase: false,
        }
    }
}

impl Module for Colors {
    fn id(&self) -> &'static str {
        "colors"
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
            ContributionSpec::new("hub", "card", "recent", "Card", "Colors")
                .icon("palette")
                .order(45)
                .options(json!({ "span": 1 })),
            ContributionSpec::new("hub", "page", "history", "Page", "Colors")
                .icon("palette")
                .order(45),
            ContributionSpec::new("launcher", "provider", "colors", "", "Colors").options(json!({
                "prefix": "#",
                "search": "search",
                "pick": "pick-result",
            })),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let color = || ArgSpec::string("color", "A color, like #1e1e2e");
        let point = |spec: ActionSpec| {
            spec.arg(ArgSpec::string("output", "The monitor"))
                .arg(ArgSpec::int(
                    "x",
                    "In the monitor's own pixels, from the left",
                ))
                .arg(ArgSpec::int(
                    "y",
                    "In the monitor's own pixels, from the top",
                ))
        };
        vec![
            ActionSpec::new("pick", "Pick a color from the screen"),
            ActionSpec::new(
                "start",
                "Close the hub, then pick a color; the hub sends this",
            ),
            ActionSpec::new("cancel", "Close the picker"),
            point(ActionSpec::new(
                "hover",
                "Show the pixels around a point in the magnifier; the overlay sends this",
            )),
            point(ActionSpec::new(
                "select",
                "Pick the color at a point; the overlay sends this",
            )),
            ActionSpec::new("copy", "Copy a color, in the default format or another")
                .arg(color())
                .arg(
                    ArgSpec::choice(
                        "format",
                        "The format; `format` when left out",
                        Format::NAMES,
                    )
                    .optional(),
                ),
            ActionSpec::new("add", "Add a color to the history").arg(color()),
            ActionSpec::new("remove", "Remove a color from the history").arg(color()),
            ActionSpec::new("clear", "Remove every color from the history"),
            ActionSpec::new(
                "search",
                "List a color in every format, as JSON lines; the launcher sends this",
            )
            .arg(
                ArgSpec::string("query", "A color, or nothing for the history")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new(
                "pick-result",
                "Keep a color the launcher listed in the history, or open the picker for `pick`",
            )
            .arg(ArgSpec::string("id", "A color, or pick")),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let path = history::state_file();
            let history = match &path {
                Some(path) => History::load(path, settings.history),
                None => History::new(settings.history),
            };
            let mut state = State {
                settings,
                history,
                path,
                picker: None,
                card: None,
                open_at: None,
            };
            state.publish(&ctx);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, .. }) => state.ended(&ctx, activity),
                        Some(_) => {}
                    },
                    () = opening(state.open_at) => {
                        state.open_at = None;
                        // open logs why it failed; nobody waits for an answer.
                        let _ = state.open(&ctx).await;
                    }
                }
            }
        })
    }
}

/// Waits until the picker is due to open, or forever when it isn't.
async fn opening(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

#[derive(Debug)]
struct State {
    settings: Settings,
    history: History,
    /// `None` without a home directory: the history then lasts until a
    /// restart.
    path: Option<PathBuf>,
    picker: Option<Picker>,
    card: Option<ActivityId>,
    /// When the picker opens, once the hub or the launcher has gone.
    open_at: Option<Instant>,
}

impl State {
    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let color = || {
            let text = args.str("color").unwrap_or_default();
            Color::parse(text).ok_or_else(|| format!("{text} isn't a color"))
        };
        let result = match command.action.as_str() {
            "search" => {
                let lines = search(
                    &self.history,
                    &self.settings,
                    args.str("query").unwrap_or_default(),
                );
                let text: Vec<String> = lines.iter().map(Value::to_string).collect();
                command.answer(Ok(text.join("\n")));
                return;
            }
            "pick" if self.card.is_some() => {
                self.open_later(ctx);
                Ok(())
            }
            "pick" => self.open(ctx).await,
            "start" => {
                close_hub(ctx);
                self.open_later(ctx);
                Ok(())
            }
            "cancel" => {
                self.close(ctx);
                Ok(())
            }
            "hover" => self.hover(ctx, args),
            "select" => self.select(ctx, args),
            "copy" => color().map(|color| {
                let format = args
                    .str("format")
                    .and_then(Format::parse)
                    .unwrap_or(self.settings.format);
                copy(ctx, color.format(format, self.settings.uppercase));
            }),
            "add" => color().map(|color| {
                self.history.add(color);
                self.changed(ctx);
            }),
            "remove" => color().and_then(|color| {
                if self.history.remove(color) {
                    self.changed(ctx);
                    Ok(())
                } else {
                    Err(format!("{} isn't in the history", color.id()))
                }
            }),
            "clear" => {
                self.history.clear();
                self.changed(ctx);
                Ok(())
            }
            "pick-result" => match args.str("id").unwrap_or_default() {
                "pick" => {
                    // The launcher closes itself; the hub may be open too.
                    close_hub(ctx);
                    self.open_later(ctx);
                    Ok(())
                }
                id => match Color::parse(id) {
                    Some(color) => {
                        self.history.add(color);
                        self.changed(ctx);
                        Ok(())
                    }
                    None => Err(format!("{id} isn't a color")),
                },
            },
            other => Err(format!("colors has no action {other}")),
        };
        if let Err(message) = &result {
            tracing::debug!(action = %command.action, %message, "colors");
        }
        command.reply(result);
    }

    /// Copies every screen, then opens the picker over them.
    async fn open(&mut self, ctx: &ModuleCtx) -> Result<(), String> {
        self.open_at = None;
        if self.picker.is_some() {
            return Ok(());
        }
        let dir = ctx.data_dir().to_owned();
        let frames = tokio::task::spawn_blocking(move || screen::capture(&dir))
            .await
            .unwrap_or_else(|error| Err(error.to_string()))
            .inspect_err(|error| tracing::warn!(%error, "cannot read the screens"))?;
        let picker = Picker {
            activity: ActivityId(0),
            frames,
            uppercase: self.settings.uppercase,
            lens: Value::Null,
        };
        let spec = ActivitySpec::new("Picking")
            .key("picker")
            // Over anything: the frozen screen still shows the rest.
            .priority(Priority::TOP)
            .uninterruptible()
            .overlay("Overlay")
            .payload(picker.payload());
        self.picker = Some(Picker {
            activity: ctx.present(spec),
            ..picker
        });
        self.publish(ctx);
        Ok(())
    }

    /// The pixels around the pointer, for the magnifier.
    fn hover(&mut self, ctx: &ModuleCtx, args: &Args) -> Result<(), String> {
        let picker = self.picker.as_mut().ok_or("the picker isn't open")?;
        let (frame, x, y) = picker.point(args)?;
        picker.lens = lens(frame, x, y, picker.uppercase);
        ctx.update(picker.activity, picker.payload());
        Ok(())
    }

    fn select(&mut self, ctx: &ModuleCtx, args: &Args) -> Result<(), String> {
        let picker = self.picker.as_ref().ok_or("the picker isn't open")?;
        let (frame, x, y) = picker.point(args)?;
        let color = frame.pixel(x, y).ok_or("that's off the screen")?;
        self.picked(ctx, color);
        Ok(())
    }

    /// Opens the picker once the island is clear: the frozen frame would
    /// show what goes.
    fn open_later(&mut self, ctx: &ModuleCtx) {
        if let Some(card) = self.card.take() {
            ctx.withdraw(card);
        }
        self.open_at = Some(Instant::now() + CLEARS);
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        self.open_at = None;
        if let Some(picker) = self.picker.take() {
            ctx.withdraw(picker.activity);
            self.publish(ctx);
        }
    }

    fn ended(&mut self, ctx: &ModuleCtx, activity: ActivityId) {
        if self
            .picker
            .as_ref()
            .is_some_and(|picker| picker.activity == activity)
        {
            self.picker = None;
            self.publish(ctx);
        }
        if self.card == Some(activity) {
            self.card = None;
        }
    }

    /// A color is picked: copy it, keep it, show it.
    fn picked(&mut self, ctx: &ModuleCtx, color: Color) {
        self.close(ctx);
        let text = color.format(self.settings.format, self.settings.uppercase);
        tracing::info!(color = %text, "picked");
        copy(ctx, text);
        self.history.add(color);
        self.changed(ctx);
        let spec = ActivitySpec::new("Picked")
            .key("picked")
            .priority(Priority::HIGH)
            .timeout(CARD)
            .payload(entry(color, &self.settings));
        self.card = Some(ctx.present(spec));
    }

    /// Saves the history and tells the views.
    fn changed(&self, ctx: &ModuleCtx) {
        if let Some(path) = &self.path
            && let Err(error) = self.history.save(path)
        {
            tracing::warn!(%error, path = %path.display(), "could not save the colors");
        }
        self.publish(ctx);
    }

    fn publish(&self, ctx: &ModuleCtx) {
        ctx.publish_state(state(&self.history, &self.settings, self.picker.is_some()));
    }
}

/// The picker on screen, with a copy of every screen.
#[derive(Debug)]
struct Picker {
    activity: ActivityId,
    frames: Vec<Frame>,
    uppercase: bool,
    /// What the magnifier shows, from the last `hover`.
    lens: Value,
}

impl Picker {
    /// The overlays get each screen's size in its own pixels, to turn the
    /// pointer's logical position into one, and the magnifier.
    fn payload(&self) -> Value {
        let screens: serde_json::Map<String, Value> = self
            .frames
            .iter()
            .map(|frame| {
                let (width, height) = frame.size();
                (
                    frame.output.clone(),
                    json!({ "width": width, "height": height }),
                )
            })
            .collect();
        json!({ "screens": screens, "cells": LENS, "lens": self.lens })
    }

    /// The screen and the point an overlay named.
    fn point(&self, args: &Args) -> Result<(&Frame, i64, i64), String> {
        let output = args.str("output").unwrap_or_default();
        let frame = self
            .frames
            .iter()
            .find(|frame| frame.output == output)
            .ok_or_else(|| format!("no copy of {output}"))?;
        Ok((
            frame,
            args.int("x").unwrap_or_default(),
            args.int("y").unwrap_or_default(),
        ))
    }
}

/// The magnifier's pixels around `x`, `y`, row by row, as `#rrggbb` or ""
/// off the screen, and the hex of the one in the middle.
fn lens(frame: &Frame, x: i64, y: i64, uppercase: bool) -> Value {
    let half = LENS / 2;
    let cells: Vec<String> = (y - half..=y + half)
        .flat_map(|row| (x - half..=x + half).map(move |column| (column, row)))
        .map(|(column, row)| frame.pixel(column, row).map(Color::id).unwrap_or_default())
        .collect();
    let center = frame.pixel(x, y);
    json!({
        "output": frame.output,
        "x": x,
        "y": y,
        "cells": cells,
        "hex": center.map(|color| color.format(Format::Hex, uppercase)),
        "swatch": center.map(Color::id),
    })
}

/// The module's state, for the hub.
fn state(history: &History, settings: &Settings, picking: bool) -> Value {
    let colors: Vec<Value> = history
        .colors()
        .iter()
        .map(|color| entry(*color, settings))
        .collect();
    json!({
        "format": settings.format.name(),
        "picking": picking,
        "history": colors,
    })
}

/// One color for the views: its id, the color QML draws, the text in the
/// default format, and every format.
fn entry(color: Color, settings: &Settings) -> Value {
    let formats: Vec<Value> = Format::ALL
        .into_iter()
        .map(|format| {
            json!({
                "format": format.name(),
                "label": format.label(),
                "text": color.format(format, settings.uppercase),
            })
        })
        .collect();
    json!({
        "color": color.id(),
        "swatch": color.qml(),
        "text": color.format(settings.format, settings.uppercase),
        "formats": formats,
    })
}

/// The launcher's results for what follows `#`: with nothing, a result
/// that opens the picker and the history; with a color, that color in each
/// format; with anything else, the colors in the history and the CSS names
/// that start with it.
fn search(history: &History, settings: &Settings, query: &str) -> Vec<Value> {
    let query = query.trim();
    // Enter copies the text, Shift+Enter types it into the window.
    let result = |title: String, subtitle: &str, color: Color| {
        json!({
            "title": title,
            "subtitle": subtitle,
            "color": color.id(),
            "copy": title,
            "alt": { "type": title },
            "id": color.id(),
        })
    };
    let recent = |color: &Color| {
        let text = color.format(settings.format, settings.uppercase);
        result(text, "Recent", *color)
    };
    if query.is_empty() {
        let pick = json!({
            "title": "Pick a color from the screen",
            "subtitle": "Click a pixel, Escape cancels",
            "id": "pick",
        });
        return std::iter::once(pick)
            .chain(history.colors().iter().map(recent))
            .collect();
    }
    if let Some(color) = Color::parse(query) {
        let mut lines: Vec<Value> = Format::ALL
            .into_iter()
            .map(|format| {
                result(
                    color.format(format, settings.uppercase),
                    format.label(),
                    color,
                )
            })
            .collect();
        if let Some(name) = color::name_of(color)
            && !query.eq_ignore_ascii_case(name)
        {
            lines.push(result(name.to_owned(), "CSS name", color));
        }
        return lines;
    }
    let lower = query.to_ascii_lowercase();
    let lower = lower.trim_start_matches('#');
    history
        .colors()
        .iter()
        .filter(|color| color.id()[1..].starts_with(lower))
        .map(recent)
        .chain(
            color::names_starting(lower)
                .take(NAMES_LISTED)
                .map(|(name, color)| result(name.to_owned(), &color.id(), color)),
        )
        .collect()
}

/// Copies through the clipboard module, so the history has it, or with
/// wl-copy when it isn't enabled. Not awaited: the clipboard never calls
/// back, but a slow one shouldn't hold up the picker.
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
            Err(error) => tracing::warn!(%error, "the clipboard module couldn't copy"),
        }
    });
}

/// Closes the hub, so the screen is clear when it freezes.
fn close_hub(ctx: &ModuleCtx) {
    let close = ctx.call("hub", "close", &[]);
    tokio::spawn(async move {
        match close.await {
            Ok(()) | Err(CallError::NotEnabled(_)) => {}
            Err(error) => tracing::warn!(%error, "could not close the hub"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(colors: &[&str]) -> History {
        let mut history = History::new(10);
        for text in colors.iter().rev() {
            history.add(Color::parse(text).unwrap());
        }
        history
    }

    fn titles(lines: &[Value]) -> Vec<&str> {
        lines
            .iter()
            .map(|line| line["title"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn nothing_typed_offers_the_picker_then_the_history() {
        let settings = Settings::default();
        let lines = search(&history(&["#1e1e2e", "#ff0000"]), &settings, "  ");
        assert_eq!(
            lines[0],
            json!({
                "title": "Pick a color from the screen",
                "subtitle": "Click a pixel, Escape cancels",
                "id": "pick",
            })
        );
        assert_eq!(titles(&lines[1..]), ["#1e1e2e", "#ff0000"]);
        assert_eq!(lines[1]["copy"], "#1e1e2e");
        assert_eq!(lines[1]["color"], "#1e1e2e");

        let settings = Settings {
            format: Format::Rgb,
            ..Settings::default()
        };
        let lines = search(&history(&["#1e1e2e"]), &settings, "");
        assert_eq!(lines[1]["title"], "rgb(30, 30, 46)");
        assert_eq!(lines[1]["id"], "#1e1e2e");
    }

    #[test]
    fn a_color_typed_lists_every_format() {
        let settings = Settings::default();
        let lines = search(&History::new(10), &settings, "1e1e2e");
        assert_eq!(
            lines[0],
            json!({
                "title": "#1e1e2e",
                "subtitle": "HEX",
                "color": "#1e1e2e",
                "copy": "#1e1e2e",
                "alt": { "type": "#1e1e2e" },
                "id": "#1e1e2e",
            })
        );
        assert_eq!(
            titles(&lines),
            [
                "#1e1e2e",
                "rgb(30, 30, 46)",
                "hsl(240, 21%, 15%)",
                "oklch(24% 0.03 284)"
            ]
        );
        for query in ["rgb(30 30 46)", "rgb(30, 30, 46)", "hsl(240 21% 15%)"] {
            let lines = search(&History::new(10), &settings, query);
            assert_eq!(lines[0]["id"], "#1e1e2e", "{query}");
        }
        // A named color says its name; typing the name doesn't repeat it.
        let lines = search(&History::new(10), &settings, "#663399");
        assert_eq!(lines.last().unwrap()["title"], "rebeccapurple");
        let lines = search(&History::new(10), &settings, "rebeccapurple");
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0]["title"], "#663399");

        let upper = Settings {
            uppercase: true,
            ..Settings::default()
        };
        let lines = search(&History::new(10), &upper, "#abc");
        assert_eq!(lines[0]["title"], "#AABBCC");
        assert_eq!(lines[0]["id"], "#aabbcc");
    }

    #[test]
    fn a_partial_color_lists_what_starts_with_it() {
        let settings = Settings::default();
        let lines = search(&history(&["#1e1e2e", "#1e2030"]), &settings, "1e1e2");
        assert_eq!(titles(&lines), ["#1e1e2e"]);
        let lines = search(&History::new(10), &settings, "rebecca");
        assert_eq!(titles(&lines), ["rebeccapurple"]);
        assert_eq!(lines[0]["subtitle"], "#663399");
        let lines = search(&History::new(10), &settings, "l");
        assert_eq!(lines.len(), NAMES_LISTED);
        assert!(search(&History::new(10), &settings, "zzz").is_empty());
    }

    #[test]
    fn the_lens_shows_the_pixels_around_the_pointer() {
        let frame = Frame::solid("DP-1", 20, 10, Color::rgb(0x1e, 0x1e, 0x2e));
        let lens = lens(&frame, 0, 9, true);
        assert_eq!(lens["output"], "DP-1");
        assert_eq!(lens["hex"], "#1E1E2E");
        assert_eq!(lens["swatch"], "#1e1e2e");
        let cells = lens["cells"].as_array().unwrap();
        assert_eq!(cells.len(), (LENS * LENS) as usize);
        // In a corner: only the quarter down and right of it is on screen.
        let middle = (LENS / 2 * LENS + LENS / 2) as usize;
        assert_eq!(cells[middle], "#1e1e2e");
        assert_eq!(cells[middle - 1], "");
        assert_eq!(cells[middle + LENS as usize], "");
        assert_eq!(cells[middle - LENS as usize + 1], "#1e1e2e");
        assert_eq!(super::lens(&frame, 30, 30, false)["hex"], Value::Null);
    }

    #[test]
    fn the_state_has_every_format() {
        let settings = Settings {
            uppercase: true,
            ..Settings::default()
        };
        let state = state(&history(&["#1e1e2e80"]), &settings, true);
        assert_eq!(state["picking"], true);
        assert_eq!(state["format"], "hex");
        let color = &state["history"][0];
        assert_eq!(color["color"], "#1e1e2e80");
        assert_eq!(color["swatch"], "#801e1e2e");
        assert_eq!(color["text"], "#1E1E2E80");
        assert_eq!(
            color["formats"][3],
            json!({ "format": "oklch", "label": "OKLCH", "text": "oklch(24% 0.03 284 / 0.5)" })
        );
    }

    #[test]
    fn actions_include_add() {
        let colors = Colors;
        let actions = colors.actions();
        assert!(actions.iter().any(|a| a.name == "add"));
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "colors",
            include_str!("../settings.toml"),
        );
    }
}

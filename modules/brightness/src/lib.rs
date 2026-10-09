//! Brightness: the laptop's backlight, the keyboard's, and external monitors
//! over DDC/CI.
//!
//! The backlight comes from `/sys/class/backlight` and the keyboard's from
//! `/sys/class/leds`, and both change through logind; a change from
//! elsewhere, like a key the firmware handles, shows the OSD too. Monitors
//! come from `ddcutil`, found once at the start and again on `mochi ipc
//! brightness refresh`. The control center has a card with a slider for
//! each, and `up`, `down` and `set` change them from keybinds. `all` means
//! every display; the keyboard changes only when named.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.brightness]
//! step = 5          # the percent up and down move
//! external = true   # monitors over DDC/CI, through ddcutil
//! osd = true        # the level on the island when it changes
//! ```

mod backlight;
mod ddc;
mod tour;

use std::path::Path;
use std::time::{Duration, Instant};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivitySpec, ArgSpec, Assets, BoxFuture, ContributionSpec, Module, ModuleCommand,
    ModuleCtx, ModuleError, ModuleEvent, Priority, SamePriority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use zbus::Connection;

use crate::backlight::Backlight;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

const OSD_TIMEOUT: Duration = Duration::from_millis(1500);
/// How often the backlights are read for changes made elsewhere.
const POLL: Duration = Duration::from_millis(500);
/// After setting a backlight, how long a different value read back is ours
/// still settling rather than someone else's change.
const SETTLING: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub struct Brightness;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// The percent `up` and `down` move.
    #[schemars(range(min = 1, max = 50))]
    step: u32,
    /// Monitors over DDC/CI, through ddcutil.
    external: bool,
    /// The level on the island when it changes.
    osd: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            step: 5,
            external: true,
            osd: true,
        }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if !(1..=50).contains(&settings.step) {
            return Err(format!("step is {}; it goes from 1 to 50", settings.step));
        }
        Ok(settings)
    }
}

impl Module for Brightness {
    fn id(&self) -> &'static str {
        "brightness"
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
        Settings::load(table).map(drop)
    }

    fn needs(&self, table: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        let external = Settings::load(table).is_ok_and(|settings| settings.external);
        if external {
            vec![mochi_core::Need::new(
                "ddcutil",
                "External monitors' brightness, with the i2c-dev module and access to /dev/i2c-*",
            )]
        } else {
            Vec::new()
        }
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("control-center", "card", "levels", "Card", "Brightness")
                .icon("light_mode")
                .order(12)
                .options(json!({ "span": 2 })),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let display = || {
            ArgSpec::string(
                "display",
                "all, backlight, keyboard, external, or a monitor's output or model; all displays by default, without the keyboard",
            )
            .optional()
            .source("brightness-display")
        };
        vec![
            ActionSpec::new("up", "Brighter by the step").arg(display()),
            ActionSpec::new("down", "Dimmer by the step").arg(display()),
            ActionSpec::new("set", "Set the brightness")
                .arg(ArgSpec::string(
                    "level",
                    "A percent like 40, or +5 and -5 to move it",
                ))
                .arg(display()),
            ActionSpec::new("refresh", "Look for monitors again"),
            ActionSpec::new(
                "status",
                "Print each display's brightness, and the keyboard's",
            ),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused a step out of range.
            let settings: Settings = ctx.settings()?;
            let (found, mut detected) = mpsc::unbounded_channel();
            let mut state = State {
                step: settings.step,
                osd: settings.osd,
                external: settings.external,
                logind: Connection::system().await.ok(),
                backlight: backlight::find(Path::new(backlight::SYSFS)),
                keyboard: backlight::find_keyboard(Path::new(backlight::LEDS)),
                settled: [Instant::now(); 2],
                monitors: Vec::new(),
                detecting: false,
                found,
            };
            state.detect();
            state.publish(&ctx);
            let polled = state.backlight.is_some() || state.keyboard.is_some();
            let mut poll = tokio::time::interval(POLL);
            poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(_) => {}
                    },
                    Some(result) = detected.recv() => {
                        state.detecting = false;
                        match result {
                            Ok(found) => {
                                tracing::debug!(monitors = found.len(), "found monitors over DDC/CI");
                                state.monitors = found.into_iter().map(Monitor::new).collect();
                            }
                            Err(error) => tracing::info!(%error, "no monitors over DDC/CI"),
                        }
                        state.publish(&ctx);
                    }
                    _ = poll.tick(), if polled => {
                        state.poll(&ctx, Which::Backlight);
                        state.poll(&ctx, Which::Keyboard);
                    }
                }
            }
        })
    }
}

#[derive(Debug)]
struct Monitor {
    found: ddc::Found,
    writer: watch::Sender<u32>,
}

impl Monitor {
    fn new(found: ddc::Found) -> Self {
        let writer = ddc::writer(found.bus, found.value);
        Self { found, writer }
    }

    fn id(&self) -> String {
        self.found
            .output
            .clone()
            .unwrap_or_else(|| format!("i2c-{}", self.found.bus))
    }

    fn percent(&self) -> u32 {
        backlight::percent(self.found.value, self.found.max)
    }

    fn set(&mut self, percent: u32) {
        let max = f64::from(self.found.max);
        let value = (f64::from(percent.min(100)) * max / 100.0).round() as u32;
        self.found.value = value;
        self.writer.send_replace(value);
    }
}

/// A display a command names, or the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Backlight,
    Keyboard,
    Monitor(usize),
}

#[derive(Debug)]
struct State {
    step: u32,
    osd: bool,
    external: bool,
    logind: Option<Connection>,
    backlight: Option<Backlight>,
    keyboard: Option<Backlight>,
    /// Until when the backlight, then the keyboard's, may still read back an
    /// older value of ours.
    settled: [Instant; 2],
    monitors: Vec<Monitor>,
    detecting: bool,
    found: mpsc::UnboundedSender<Result<Vec<ddc::Found>, String>>,
}

impl State {
    /// Looks for monitors in the background.
    fn detect(&mut self) {
        if !self.external || self.detecting {
            return;
        }
        self.detecting = true;
        let found = self.found.clone();
        tokio::spawn(async move {
            let _ = found.send(ddc::detect().await);
        });
    }

    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let display = command.args.str("display").unwrap_or("all").to_owned();
        let level = match command.action.as_str() {
            "up" => format!("+{}", self.step),
            "down" => format!("-{}", self.step),
            "set" => command.args.str("level").unwrap_or_default().to_owned(),
            "refresh" => {
                let result = if self.external {
                    self.detect();
                    self.publish(ctx);
                    Ok(())
                } else {
                    Err("external is off in the settings".to_owned())
                };
                return command.reply(result);
            }
            "status" => return command.answer(Ok(self.status())),
            other => {
                let error = format!("brightness has no action {other}");
                return command.reply(Err(error));
            }
        };
        let result = self.change(ctx, &display, &level).await;
        command.reply(result);
    }

    /// Sets `display` to `level`, then shows the first display changed.
    async fn change(&mut self, ctx: &ModuleCtx, display: &str, level: &str) -> Result<(), String> {
        let targets = self.targets(display)?;
        let moved = level.trim_start().starts_with(['+', '-']);
        let mut shown = None;
        for which in targets {
            let now = self.percent(which);
            let percent = parse_level(level, now)?;
            match which {
                Which::Backlight | Which::Keyboard => {
                    let logind = self.logind.clone();
                    let Some(light) = self.light_mut(which) else {
                        continue;
                    };
                    let value = if moved {
                        light.value_moved_to(percent)
                    } else {
                        light.value_for(percent)
                    };
                    backlight::set(logind.as_ref(), light, value).await?;
                    light.value = value;
                    *self.settled_mut(which) = Instant::now() + SETTLING;
                }
                Which::Monitor(index) => self.monitors[index].set(percent),
            }
            shown.get_or_insert(which);
        }
        self.publish(ctx);
        if let Some(which) = shown {
            self.show(ctx, which);
        }
        Ok(())
    }

    fn light_mut(&mut self, which: Which) -> Option<&mut Backlight> {
        match which {
            Which::Backlight => self.backlight.as_mut(),
            Which::Keyboard => self.keyboard.as_mut(),
            Which::Monitor(_) => None,
        }
    }

    fn settled_mut(&mut self, which: Which) -> &mut Instant {
        &mut self.settled[usize::from(which == Which::Keyboard)]
    }

    /// The displays `name` stands for, or the keyboard.
    fn targets(&self, name: &str) -> Result<Vec<Which>, String> {
        let backlight = self.backlight.iter().map(|_| Which::Backlight);
        let monitors = (0..self.monitors.len()).map(Which::Monitor);
        let targets: Vec<Which> = match name.to_lowercase().as_str() {
            "all" | "" => backlight.chain(monitors).collect(),
            "backlight" | "builtin" | "internal" => backlight.collect(),
            "keyboard" | "kbd" => match self.keyboard {
                Some(_) => vec![Which::Keyboard],
                None => return Err("no keyboard backlight".to_owned()),
            },
            "external" | "monitors" | "ddc" => monitors.collect(),
            lower => monitors
                .filter(|which| {
                    let Which::Monitor(index) = which else {
                        return false;
                    };
                    let monitor = &self.monitors[*index];
                    monitor.id().to_lowercase() == lower
                        || monitor.found.model.to_lowercase() == lower
                })
                .collect(),
        };
        if targets.is_empty() {
            return Err(if self.detecting {
                "still looking for monitors".to_owned()
            } else if name == "all" {
                "no backlight, and no monitor answers DDC/CI".to_owned()
            } else {
                format!("no display called {name}")
            });
        }
        Ok(targets)
    }

    fn percent(&self, which: Which) -> u32 {
        match which {
            Which::Backlight => self.backlight.as_ref().map_or(0, Backlight::percent),
            Which::Keyboard => self.keyboard.as_ref().map_or(0, Backlight::percent),
            Which::Monitor(index) => self.monitors[index].percent(),
        }
    }

    /// Reads the backlight or the keyboard's for a change made elsewhere.
    fn poll(&mut self, ctx: &ModuleCtx, which: Which) {
        let settled = *self.settled_mut(which);
        let Some(light) = self.light_mut(which) else {
            return;
        };
        let Some(value) = light.read() else {
            return;
        };
        if value == light.value || Instant::now() < settled {
            return;
        }
        light.value = value;
        self.publish(ctx);
        self.show(ctx, which);
    }

    fn show(&self, ctx: &ModuleCtx, which: Which) {
        if !self.osd {
            return;
        }
        let (name, icon) = match which {
            Which::Backlight => ("Built-in".to_owned(), "light_mode"),
            Which::Keyboard => ("Keyboard".to_owned(), "keyboard"),
            Which::Monitor(index) => (self.monitors[index].found.model.clone(), "desktop_windows"),
        };
        let spec = ActivitySpec::new("Osd")
            .key("osd")
            .priority(Priority::HIGH)
            .same_priority(SamePriority::Stack)
            // Feedback for a key just pressed, like the volume's.
            .passive()
            .fleeting()
            .timeout(OSD_TIMEOUT)
            .payload(json!({
                "display": self.id(which),
                "name": name,
                "icon": icon,
                "percent": self.percent(which),
            }));
        ctx.present(spec);
    }

    fn id(&self, which: Which) -> String {
        match which {
            Which::Backlight => "backlight".to_owned(),
            Which::Keyboard => "keyboard".to_owned(),
            Which::Monitor(index) => self.monitors[index].id(),
        }
    }

    fn payload(&self) -> Value {
        let mut displays = Vec::new();
        if self.backlight.is_some() {
            displays.push(json!({
                "id": "backlight",
                "name": "Built-in",
                "icon": "laptop",
                "percent": self.percent(Which::Backlight),
            }));
        }
        for (index, monitor) in self.monitors.iter().enumerate() {
            displays.push(json!({
                "id": monitor.id(),
                "name": monitor.found.model,
                "icon": "desktop_windows",
                "percent": self.percent(Which::Monitor(index)),
            }));
        }
        // Apart from the displays, as `all` leaves it alone.
        let keyboard = self.keyboard.as_ref().map(|keyboard| {
            json!({
                "id": "keyboard",
                "name": "Keyboard",
                "icon": "keyboard",
                "percent": keyboard.percent(),
                // Its levels past off, often two or three, for the slider
                // to stop at.
                "levels": keyboard.max,
            })
        });
        json!({
            "displays": displays,
            "keyboard": keyboard,
            "detecting": self.detecting,
            "step": self.step,
        })
    }

    fn publish(&self, ctx: &ModuleCtx) {
        ctx.publish_state(self.payload());
    }

    fn status(&self) -> String {
        let mut lines = Vec::new();
        if let Some(backlight) = &self.backlight {
            lines.push(format!(
                "backlight ({}): {}%",
                backlight.name,
                backlight.percent()
            ));
        }
        for monitor in &self.monitors {
            lines.push(format!(
                "{} ({}): {}%",
                monitor.id(),
                monitor.found.model,
                monitor.percent()
            ));
        }
        if self.detecting {
            lines.push("looking for monitors".to_owned());
        }
        if lines.is_empty() {
            lines.push("no backlight, and no monitor answers DDC/CI".to_owned());
        }
        if let Some(keyboard) = &self.keyboard {
            lines.push(format!(
                "keyboard ({}): {}%, level {} of {}",
                keyboard.name,
                keyboard.percent(),
                keyboard.value,
                keyboard.max
            ));
        }
        lines.join("\n")
    }
}

/// A level as a command gives it, `40`, `40%`, `+5` or `-5`, from `now`.
fn parse_level(text: &str, now: u32) -> Result<u32, String> {
    let text = text.trim().trim_end_matches('%');
    let number = |digits: &str| {
        digits
            .parse::<u32>()
            .map_err(|_| format!("{text} isn't a level; give a percent like 40, or +5 and -5"))
    };
    let level = if let Some(up) = text.strip_prefix('+') {
        now.saturating_add(number(up)?)
    } else if let Some(down) = text.strip_prefix('-') {
        now.saturating_sub(number(down)?)
    } else {
        number(text)?
    };
    Ok(level.min(100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_set_or_moved() {
        assert_eq!(parse_level("40", 10), Ok(40));
        assert_eq!(parse_level("40%", 10), Ok(40));
        assert_eq!(parse_level("+5", 10), Ok(15));
        assert_eq!(parse_level("-15", 10), Ok(0));
        assert_eq!(parse_level("+20", 95), Ok(100));
        assert_eq!(parse_level("250", 0), Ok(100));
        assert!(parse_level("bright", 0).is_err());
    }

    /// A laptop from fake sysfs directories, with a keyboard backlight of
    /// two levels at the first.
    fn laptop(root: &Path) -> State {
        for (dir, name, value, max) in [
            ("backlight", "intel_backlight", 600, 1200),
            ("leds", "tpacpi::kbd_backlight", 1, 2),
            ("leds", "input3::capslock", 0, 1),
        ] {
            let path = root.join(dir).join(name);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("brightness"), format!("{value}\n")).unwrap();
            std::fs::write(path.join("max_brightness"), format!("{max}\n")).unwrap();
        }
        std::fs::write(root.join("backlight/intel_backlight/type"), "raw\n").unwrap();
        State {
            step: 5,
            osd: true,
            external: false,
            logind: None,
            backlight: backlight::find(&root.join("backlight")),
            keyboard: backlight::find_keyboard(&root.join("leds")),
            settled: [Instant::now(); 2],
            monitors: Vec::new(),
            detecting: false,
            found: mpsc::unbounded_channel().0,
        }
    }

    #[test]
    fn the_keyboard_changes_only_when_named() {
        let root = std::env::temp_dir().join(format!("mochi-brightness-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let state = laptop(&root);
        assert_eq!(state.targets("all"), Ok(vec![Which::Backlight]));
        assert_eq!(state.targets("keyboard"), Ok(vec![Which::Keyboard]));
        assert_eq!(state.targets("kbd"), Ok(vec![Which::Keyboard]));
        let payload = state.payload();
        assert_eq!(payload["displays"].as_array().unwrap().len(), 1);
        assert_eq!(payload["keyboard"]["percent"], 50);
        assert_eq!(payload["keyboard"]["levels"], 2);
        assert_eq!(
            state.status(),
            "backlight (intel_backlight): 50%\nkeyboard (tpacpi::kbd_backlight): 50%, level 1 of 2"
        );
        std::fs::remove_dir_all(&root).unwrap();
        let state = laptop(&root.join("none"));
        std::fs::remove_dir_all(root.join("none/leds")).unwrap();
        let desktop = State {
            keyboard: backlight::find_keyboard(&root.join("none/leds")),
            ..state
        };
        assert_eq!(
            desktop.targets("keyboard"),
            Err("no keyboard backlight".to_owned())
        );
        assert_eq!(desktop.payload()["keyboard"], Value::Null);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<Settings>(
            "brightness",
            include_str!("../settings.toml"),
        );
    }

    #[test]
    fn the_step_stays_in_range() {
        let table = |text: &str| mochi_core::toml::from_str(text).unwrap();
        assert!(Settings::load(&table("step = 10")).is_ok());
        assert!(Settings::load(&table("step = 0")).is_err());
        assert!(Settings::load(&table("step = 51")).is_err());
    }
}

//! Where Mochi keeps its files, and the two files users edit: `config.toml`
//! and `theme.toml`. A missing file means defaults. A file with a typo is an
//! error that names the file and the key.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use mochi_protocol::Theme;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::bubbles::Placement;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{}: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
    #[error("{0} is not set")]
    MissingVar(&'static str),
}

impl ConfigError {
    pub fn invalid(path: &Path, message: impl Into<String>) -> Self {
        Self::Invalid {
            path: path.to_owned(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// `$XDG_CONFIG_HOME/mochi`
    pub config_dir: PathBuf,
    /// `$XDG_RUNTIME_DIR/mochi`
    pub runtime_dir: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_vars(|name| env::var_os(name))
    }

    /// Resolves the paths from environment variables. `XDG_CONFIG_HOME`
    /// falls back to `$HOME/.config`.
    pub fn from_vars(var: impl Fn(&str) -> Option<OsString>) -> Result<Self, ConfigError> {
        let set = |name: &str| {
            var(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };

        let config_home = match set("XDG_CONFIG_HOME") {
            Some(dir) => dir,
            None => set("HOME")
                .ok_or(ConfigError::MissingVar("HOME"))?
                .join(".config"),
        };
        let runtime = set("XDG_RUNTIME_DIR").ok_or(ConfigError::MissingVar("XDG_RUNTIME_DIR"))?;

        Ok(Self {
            config_dir: config_home.join("mochi"),
            runtime_dir: runtime.join("mochi"),
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn theme_file(&self) -> PathBuf {
        self.config_dir.join("theme.toml")
    }

    pub fn socket(&self) -> PathBuf {
        self.runtime_dir.join("mochi.sock")
    }

    /// A module's own files: [`crate::ModuleCtx::data_dir`].
    pub fn data_dir(&self, module: &str) -> PathBuf {
        self.runtime_dir.join("data").join(module)
    }

    /// A module's files that outlive a daemon restart:
    /// [`crate::ModuleCtx::session_dir`].
    pub fn session_dir(&self, module: &str) -> PathBuf {
        self.runtime_dir.join("session").join(module)
    }

    /// The generated QML tree Quickshell loads.
    pub fn shell_dir(&self) -> PathBuf {
        self.runtime_dir.join("shell")
    }
}

/// `$XDG_STATE_HOME/mochi`, falling back to `~/.local/state/mochi`: what
/// modules keep across logins, like histories.
pub fn state_dir() -> Option<PathBuf> {
    let state = env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".local/state")))?;
    Some(state.join("mochi"))
}

/// `$XDG_DATA_HOME/mochi`, falling back to `~/.local/share/mochi`: what
/// gets installed, like plugins and themes.
pub fn data_dir() -> Option<PathBuf> {
    let data = env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")))?;
    Some(data.join("mochi"))
}

/// `config.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Enabled modules, by id; every builtin one when left out.
    #[serde(default = "default_modules")]
    pub modules: Vec<String>,
    /// Settings per module: `[module.<id>]`.
    #[serde(default)]
    pub module: BTreeMap<String, toml::Table>,
    #[serde(default)]
    pub island: IslandConfig,
    #[serde(default)]
    pub bubbles: BubblesConfig,
    #[serde(default)]
    pub bento: BentoConfig,
}

/// The key that turns Bento on, long so nobody sets it without reading it.
pub const BENTO_CONSENT: &str =
    "i_really_understand_that_bento_can_harm_and_contain_malicious_content";

/// `[bento]`: off until the user says they understand what it installs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BentoConfig {
    /// Turn Bento on, knowing that what it installs can harm your computer
    /// and contain malicious content.
    #[serde(rename = "i_really_understand_that_bento_can_harm_and_contain_malicious_content")]
    pub on: bool,
}

/// `[island]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct IslandConfig {
    pub panels: Panels,
    pub notices: Notices,
    pub click_outside: ClickOutside,
}

/// Which monitor everything that isn't a panel shows on: notifications,
/// the volume, the media card, every other notice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Notices {
    /// Every monitor's island.
    #[default]
    All,
    /// The monitor with keyboard focus.
    Focus,
    /// The monitor under the pointer, where the compositor says; otherwise
    /// the focused one.
    Pointer,
}

/// What a click outside the island closes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ClickOutside {
    /// Whatever the island shows, except the idle island and quick notices
    /// like the volume.
    #[default]
    All,
    /// Only views the user opened with a click, and the ones that take the
    /// keyboard.
    Expanded,
}

/// Which monitor a panel, a view that takes the keyboard, opens on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Panels {
    /// The monitor with keyboard focus.
    #[default]
    Focus,
    /// The monitor under the pointer, where the compositor says; otherwise
    /// the focused one.
    Pointer,
    /// Every monitor.
    All,
}

/// `[bubbles]`: how many fit in an area, and where each module's bubbles go.
///
/// ```toml
/// [bubbles]
/// max_per_area = 4
///
/// [bubbles.media]     # any module id
/// area = "left"
/// group = "status"    # "" for a pill of its own
/// order = 1
/// wide = true         # the module's wide views with text, if it has them
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct BubblesConfig {
    /// Bubbles shown per area; the rest are counted instead.
    #[schemars(range(min = 1, max = 12))]
    pub max_per_area: usize,
    /// Each area stacks its bubbles into one, the most important in front.
    pub stack: bool,
    /// How long a bubble with news stays in front of its stack.
    #[schemars(range(min = 0, max = 15000))]
    pub news_ms: u64,
    /// How long the pointer rests on a bubble before a tooltip with more
    /// shows beside it; 0 never shows one.
    #[schemars(range(min = 0, max = 5000))]
    pub tooltip_ms: u64,
    #[serde(flatten)]
    #[schemars(skip)]
    pub modules: BTreeMap<String, Placement>,
}

impl Default for BubblesConfig {
    fn default() -> Self {
        Self {
            max_per_area: 4,
            stack: false,
            news_ms: 4000,
            tooltip_ms: 600,
            modules: BTreeMap::new(),
        }
    }
}

impl BubblesConfig {
    /// What the UI needs to stack, when it does.
    pub fn stacking(&self) -> Option<mochi_protocol::Stacking> {
        self.stack.then_some(mochi_protocol::Stacking {
            news_ms: self.news_ms,
        })
    }
}

/// The builtin modules a config turns on when it doesn't list `modules`,
/// and a generated `config.toml` lists: the whole shell.
pub const DEFAULT_MODULES: [&str; 30] = [
    "idle",
    "osd",
    "workspaces",
    "media",
    "audio",
    "notifications",
    "launcher",
    "control-center",
    "power",
    "capture",
    "share",
    "clipboard",
    "tray",
    "network",
    "bluetooth",
    "battery",
    "brightness",
    "privacy",
    "nightlight",
    "drop",
    "performance",
    "widgets",
    "notes",
    "emoji",
    "colors",
    "settings",
    "tour",
    "updater",
    "weather",
    "clock",
];

fn default_modules() -> Vec<String> {
    DEFAULT_MODULES.iter().map(|id| (*id).to_owned()).collect()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            modules: default_modules(),
            module: BTreeMap::new(),
            island: IslandConfig::default(),
            bubbles: BubblesConfig::default(),
            bento: BentoConfig::default(),
        }
    }
}

impl Config {
    /// Reads `path`, or returns the defaults when it doesn't exist.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match read_optional(path)? {
            Some(text) => Self::parse(&text, path),
            None => Ok(Self::default()),
        }
    }

    /// `path` only labels errors.
    pub fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let invalid = |error: toml::de::Error| ConfigError::invalid(path, error.to_string());
        let mut table: toml::Table = toml::from_str(text).map_err(invalid)?;
        if migrate_module_ids(&mut table).is_empty() {
            // Straight from the text, for errors that name the line.
            return toml::from_str(text).map_err(invalid);
        }
        Self::from_table(table, path)
    }

    /// From the file already read as a table, with changes laid over it.
    /// `path` only labels errors.
    pub fn from_table(table: toml::Table, path: &Path) -> Result<Self, ConfigError> {
        toml::Value::Table(table)
            .try_into()
            .map_err(|error: toml::de::Error| ConfigError::invalid(path, error.to_string()))
    }

    /// Checks the module names against the modules that exist.
    pub fn check(&self, available: &[&str], path: &Path) -> Result<(), ConfigError> {
        let unknown = |id: &str| {
            ConfigError::invalid(
                path,
                format!("unknown module {id:?}, available: {}", available.join(", ")),
            )
        };

        for (index, id) in self.modules.iter().enumerate() {
            if !available.contains(&id.as_str()) {
                return Err(unknown(id));
            }
            if self.modules[..index].contains(id) {
                return Err(ConfigError::invalid(
                    path,
                    format!("module {id:?} is listed twice"),
                ));
            }
        }
        for id in self.module.keys().chain(self.bubbles.modules.keys()) {
            if !available.contains(&id.as_str()) {
                return Err(unknown(id));
            }
        }
        if self.bubbles.max_per_area == 0 {
            return Err(ConfigError::invalid(
                path,
                "bubbles.max_per_area must be at least 1",
            ));
        }
        Ok(())
    }

    /// The `[module.<id>]` table, empty when there is none.
    pub fn settings(&self, module: &str) -> toml::Table {
        self.module.get(module).cloned().unwrap_or_default()
    }
}

/// Reads `theme.toml`, or returns the default theme when it doesn't exist.
pub fn load_theme(path: &Path) -> Result<Theme, ConfigError> {
    match read_optional(path)? {
        Some(text) => parse_theme(&text, path),
        None => Ok(Theme::default()),
    }
}

/// `path` only labels errors.
pub fn parse_theme(text: &str, path: &Path) -> Result<Theme, ConfigError> {
    let table: toml::Table =
        toml::from_str(text).map_err(|error| ConfigError::invalid(path, error.to_string()))?;
    let theme = theme_from_table(table, path, false)?;
    if let Ok(raw) = toml::from_str::<Theme>(text) {
        for note in raw.text.clone().migrate() {
            tracing::warn!(file = %path.display(), "{note}; the old name still works for now");
        }
    }
    Ok(theme)
}

/// `theme.toml` already read as a table, with changes laid over it: the
/// preset's palette under what `[colors]` sets. `system_light` is what the
/// system prefers, for `appearance = "auto"`. `path` only labels errors.
pub fn theme_from_table(
    mut table: toml::Table,
    path: &Path,
    system_light: bool,
) -> Result<Theme, ConfigError> {
    let colors =
        palette_of(&table, system_light).map_err(|message| ConfigError::invalid(path, message))?;
    let user = match table.remove("colors") {
        Some(toml::Value::Table(user)) => user,
        _ => toml::Table::new(),
    };
    let mut merged = colors;
    crate::changes::merge(&mut merged, &user);
    table.insert("colors".to_owned(), toml::Value::Table(merged));
    // The theme's fonts, shape and motion, under what the file sets.
    let look = preset_look(&table);
    lay_under(&mut table, look);
    let mut theme: Theme = toml::Value::Table(table)
        .try_into()
        .map_err(|error: toml::de::Error| ConfigError::invalid(path, error.to_string()))?;
    theme.text.migrate();
    check_theme(&theme).map_err(|message| ConfigError::invalid(path, message))?;
    Ok(theme)
}

/// Puts `look`'s sections under the same sections of `table`, which win.
fn lay_under(table: &mut toml::Table, look: toml::Table) {
    for (section, look) in look {
        let toml::Value::Table(mut under) = look else {
            continue;
        };
        if let Some(toml::Value::Table(over)) = table.get(&section) {
            crate::changes::merge(&mut under, over);
        }
        table.insert(section, toml::Value::Table(under));
    }
}

/// What the theme a theme table's `preset` names sets besides colors: its
/// `text`, `layout` and `motion`. Nothing for `wallpaper`, or for a theme
/// that can't be read, which `palette_of` reports.
pub fn preset_look(table: &toml::Table) -> toml::Table {
    let preset = table
        .get("preset")
        .and_then(toml::Value::as_str)
        .unwrap_or("obsidian");
    if preset == "wallpaper" {
        return toml::Table::new();
    }
    crate::themes::find(preset)
        .map(|theme| theme.look())
        .unwrap_or_default()
}

/// The palette a theme table's `preset` and `appearance` give. A
/// wallpaper that can't be read gives the default palette and a warning,
/// so a missing image doesn't stop Mochi.
pub fn palette_of(table: &toml::Table, system_light: bool) -> Result<toml::Table, String> {
    let text = |key: &str, default: &'static str| {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or(default)
            .to_owned()
    };
    let preset = text("preset", "obsidian");
    let light = match text("appearance", "dark").as_str() {
        "light" => true,
        "auto" => system_light,
        _ => false,
    };
    match crate::palette::colors(&preset, light, &text("wallpaper", "auto")) {
        Ok(colors) => Ok(colors),
        Err(error) if preset == "wallpaper" => {
            tracing::warn!(%error, "no colors from the wallpaper, using obsidian's");
            crate::palette::colors("obsidian", light, "")
        }
        Err(error) => Err(error),
    }
}

/// A TOML file as a table, empty when it doesn't exist.
pub fn read_table(path: &Path) -> Result<toml::Table, ConfigError> {
    match read_optional(path)? {
        Some(text) => {
            toml::from_str(&text).map_err(|error| ConfigError::invalid(path, error.to_string()))
        }
        None => Ok(toml::Table::new()),
    }
}

/// Moves the old ids of renamed modules in a `config.toml` table to their
/// new ones: in `modules`, `[module.<id>]`, `[bubbles.<id>]` and the
/// `<id>/<card>` names the control center's `order` and `hidden` list, where
/// the control center's own time card is now the clock's, `clock/clock`.
/// Returns a note for each old id it found, for a warning.
pub fn migrate_module_ids(table: &mut toml::Table) -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    let mut rename = |id: &str| {
        let new = mochi_protocol::module_id(id);
        if new != id {
            found.insert((id.to_owned(), new.to_owned()));
        }
        new.to_owned()
    };
    if let Some(toml::Value::Array(ids)) = table.get_mut("modules") {
        for id in ids.iter_mut() {
            if let toml::Value::String(text) = id {
                *text = rename(text);
            }
        }
    }
    for section in ["module", "bubbles"] {
        if let Some(toml::Value::Table(inner)) = table.get_mut(section) {
            for (old, new) in mochi_protocol::RENAMED_MODULES {
                if let Some(value) = inner.remove(old) {
                    rename(old);
                    // What the new name says wins over the old one.
                    match (inner.get_mut(new), value) {
                        (Some(toml::Value::Table(current)), toml::Value::Table(old)) => {
                            for (key, value) in old {
                                current.entry(key).or_insert(value);
                            }
                        }
                        (Some(_), _) => {}
                        (None, value) => {
                            inner.insert(new.to_owned(), value);
                        }
                    }
                }
            }
        }
    }
    if let Some(toml::Value::Table(center)) = table
        .get_mut("module")
        .and_then(|module| module.get_mut("control-center"))
    {
        for key in ["order", "hidden"] {
            if let Some(toml::Value::Array(names)) = center.get_mut(key) {
                for name in names.iter_mut() {
                    if let toml::Value::String(text) = name
                        && let Some((module, rest)) = text.split_once('/')
                    {
                        *text = format!("{}/{rest}", rename(module));
                        // The control center's own time card became the
                        // clock module's.
                        if text == "control-center/clock" {
                            *text = "clock/clock".to_owned();
                        }
                    }
                }
            }
        }
    }
    found
        .into_iter()
        .map(|(old, new)| format!("`{old}` is now `{new}`"))
        .collect()
}

/// The renamed keys `theme.toml` still uses, for `mochi config check`.
pub fn theme_notes(path: &Path) -> Result<Vec<String>, ConfigError> {
    Ok(match read_optional(path)? {
        Some(text) => toml::from_str::<Theme>(&text)
            .map_err(|error| ConfigError::invalid(path, error.to_string()))?
            .text
            .migrate(),
        None => Vec::new(),
    })
}

/// Rejects values the UI can't use, naming the key.
pub(crate) fn check_theme(theme: &Theme) -> Result<(), String> {
    let motion = &theme.motion;
    if !(motion.spring.is_finite() && motion.spring > 0.0) {
        return Err(format!(
            "motion.spring must be above 0, got {}",
            motion.spring
        ));
    }
    if !(motion.speed.is_finite() && motion.speed > 0.0) {
        return Err(format!(
            "motion.speed must be above 0, got {}",
            motion.speed
        ));
    }
    if !(motion.damping > 0.0 && motion.damping <= 1.0) {
        return Err(format!(
            "motion.damping must be above 0 and at most 1, got {}",
            motion.damping
        ));
    }

    let layout = &theme.layout;
    if layout.idle_height == 0 {
        return Err("layout.idle_height must be above 0".into());
    }
    if layout.margin + layout.idle_height > layout.surface_height {
        return Err(format!(
            "layout.surface_height ({}) must fit layout.margin + layout.idle_height ({})",
            layout.surface_height,
            layout.margin + layout.idle_height
        ));
    }
    Ok(())
}

fn read_optional(path: &Path) -> Result<Option<String>, ConfigError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Read {
            path: path.to_owned(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AVAILABLE: &[&str] = &["idle", "osd", "notifications"];

    fn path() -> &'static Path {
        Path::new("config.toml")
    }

    #[test]
    fn missing_files_give_defaults() {
        let missing = Path::new("/nonexistent/mochi/config.toml");
        assert_eq!(Config::load(missing).unwrap(), Config::default());
        assert_eq!(load_theme(missing).unwrap(), Theme::default());
        // Without `modules`, the whole shell.
        assert_eq!(Config::default().modules, DEFAULT_MODULES);
        assert_eq!(Config::parse("", path()).unwrap().modules, DEFAULT_MODULES);
    }

    #[test]
    fn modules_and_their_settings_parse() {
        let config = Config::parse(
            r#"
            modules = ["idle", "osd"]

            [module.osd]
            timeout_ms = 1500
            "#,
            path(),
        )
        .unwrap();

        assert_eq!(config.modules, ["idle", "osd"]);
        assert_eq!(
            config.settings("osd")["timeout_ms"].as_integer(),
            Some(1500)
        );
        assert!(config.settings("idle").is_empty());
        config.check(AVAILABLE, path()).unwrap();
    }

    #[test]
    fn bubble_placements_parse_and_name_known_modules() {
        let config = Config::parse(
            r#"
            modules = ["idle", "osd"]

            [bubbles]
            max_per_area = 3

            [bubbles.osd]
            area = "left"
            group = "status"
            "#,
            path(),
        )
        .unwrap();
        assert_eq!(config.bubbles.max_per_area, 3);
        assert_eq!(
            config.bubbles.modules["osd"],
            Placement {
                area: Some(mochi_protocol::Area::Left),
                group: Some("status".into()),
                ..Placement::default()
            }
        );
        config.check(AVAILABLE, path()).unwrap();

        let unknown = Config::parse(
            "modules = [\"idle\"]\n[bubbles.radio]\narea = \"left\"",
            path(),
        )
        .unwrap();
        let error = unknown.check(AVAILABLE, path()).unwrap_err().to_string();
        assert!(error.contains("radio"), "{error}");

        let typo = Config::parse("[bubbles.osd]\narae = \"left\"", path()).unwrap_err();
        assert!(typo.to_string().contains("arae"), "{typo}");

        let none = Config::parse("[bubbles]\nmax_per_area = 0", path()).unwrap();
        assert!(none.check(AVAILABLE, path()).is_err());
    }

    #[test]
    fn typos_name_the_file_and_the_key() {
        let error = Config::parse("moduels = [\"idle\"]", path())
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("config.toml: "), "{error}");
        assert!(error.contains("moduels"), "{error}");
    }

    #[test]
    fn unknown_and_duplicate_modules_are_rejected() {
        let config = Config::parse(r#"modules = ["idle", "spotify"]"#, path()).unwrap();
        let error = config.check(AVAILABLE, path()).unwrap_err().to_string();
        assert!(error.contains("\"spotify\""), "{error}");
        assert!(error.contains("idle, osd, notifications"), "{error}");

        let config = Config::parse(r#"modules = ["idle", "idle"]"#, path()).unwrap();
        assert!(config.check(AVAILABLE, path()).is_err());

        let config = Config::parse("[module.spotify]\nvolume = 3", path()).unwrap();
        assert!(config.check(AVAILABLE, path()).is_err());
    }

    #[test]
    fn the_hub_still_answers_to_its_old_id() {
        let mut table: toml::Table = toml::from_str(
            r#"
            modules = ["idle", "hub"]
            [module.hub]
            order = ["hub/clock", "network/card"]
            hidden = ["hub/clock"]
            [module.control-center]
            hidden = []
            [bubbles.hub]
            area = "left"
            "#,
        )
        .unwrap();
        let notes = migrate_module_ids(&mut table);
        assert_eq!(notes, ["`hub` is now `control-center`"]);
        let expected: toml::Table = toml::from_str(
            r#"
            modules = ["idle", "control-center"]
            [module.control-center]
            order = ["clock/clock", "network/card"]
            hidden = []
            [bubbles.control-center]
            area = "left"
            "#,
        )
        .unwrap();
        assert_eq!(table, expected);

        let config = Config::parse(r#"modules = ["hub"]"#, path()).unwrap();
        assert_eq!(config.modules, ["control-center"]);
    }

    #[test]
    fn theme_overrides_only_what_it_names() {
        let theme = parse_theme(
            r##"
            [colors]
            accent = "#30d158"

            [motion]
            damping = 0.5
            "##,
            Path::new("theme.toml"),
        )
        .unwrap();

        assert_eq!(theme.colors.accent.as_str(), "#30d158");
        assert_eq!(theme.motion.damping, 0.5);
        assert_eq!(theme.layout, Theme::default().layout);
    }

    #[test]
    fn a_theme_package_sits_under_the_file() {
        let mut table: toml::Table =
            toml::from_str("[text]\nfamily = \"Mine\"\n[motion]\nspeed = 2.0\n").unwrap();
        let look: toml::Table = toml::from_str(
            "[text]\nfamily = \"Theirs\"\ndisplay_family = \"Display\"\n[layout]\nmargin = 6\n",
        )
        .unwrap();
        lay_under(&mut table, look);
        let theme = theme_from_table(table, Path::new("theme.toml"), false).unwrap();
        assert_eq!(theme.text.family, "Mine");
        assert_eq!(theme.text.display_family, "Display");
        assert_eq!(theme.layout.margin, 6);
        assert!((theme.motion.speed - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn bad_theme_values_name_the_key() {
        let file = Path::new("theme.toml");
        let cases = [
            ("[motion]\ndamping = 1.5", "motion.damping"),
            ("[motion]\nspring = 0.0", "motion.spring"),
            ("[layout]\nsurface_height = 20", "layout.surface_height"),
            ("[colors]\naccent = \"orange\"", "orange"),
        ];
        for (text, expected) in cases {
            let error = parse_theme(text, file).unwrap_err().to_string();
            assert!(error.starts_with("theme.toml: "), "{error}");
            assert!(error.contains(expected), "{expected}: {error}");
        }
    }

    #[test]
    fn paths_follow_xdg() {
        let vars = |name: &str| match name {
            "XDG_CONFIG_HOME" => Some("/home/me/.cfg".into()),
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".into()),
            _ => None,
        };
        let paths = Paths::from_vars(vars).unwrap();
        assert_eq!(
            paths.config_file(),
            Path::new("/home/me/.cfg/mochi/config.toml")
        );
        assert_eq!(paths.socket(), Path::new("/run/user/1000/mochi/mochi.sock"));
        assert_eq!(paths.shell_dir(), Path::new("/run/user/1000/mochi/shell"));
    }

    #[test]
    fn config_home_falls_back_to_home() {
        let vars = |name: &str| match name {
            "XDG_CONFIG_HOME" => Some("".into()),
            "HOME" => Some("/home/me".into()),
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".into()),
            _ => None,
        };
        let paths = Paths::from_vars(vars).unwrap();
        assert_eq!(
            paths.theme_file(),
            Path::new("/home/me/.config/mochi/theme.toml")
        );
    }

    #[test]
    fn runtime_dir_is_required() {
        let vars = |name: &str| (name == "HOME").then(|| "/home/me".into());
        assert!(matches!(
            Paths::from_vars(vars),
            Err(ConfigError::MissingVar("XDG_RUNTIME_DIR"))
        ));
    }
}

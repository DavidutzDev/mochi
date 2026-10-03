//! Where Mochi keeps its files, and the two files users edit: `config.toml`
//! and `theme.toml`. A missing file means defaults. A file with a typo is an
//! error that names the file and the key.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use mochi_protocol::Theme;
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
    fn invalid(path: &Path, message: impl Into<String>) -> Self {
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

    /// The generated QML tree Quickshell loads.
    pub fn shell_dir(&self) -> PathBuf {
        self.runtime_dir.join("shell")
    }
}

/// `config.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Enabled modules, by id.
    #[serde(default = "default_modules")]
    pub modules: Vec<String>,
    /// Settings per module: `[module.<id>]`.
    #[serde(default)]
    pub module: BTreeMap<String, toml::Table>,
    #[serde(default)]
    pub bubbles: BubblesConfig,
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
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct BubblesConfig {
    /// Bubbles shown per area; the rest are counted instead.
    pub max_per_area: usize,
    #[serde(flatten)]
    pub modules: BTreeMap<String, Placement>,
}

impl Default for BubblesConfig {
    fn default() -> Self {
        Self {
            max_per_area: 4,
            modules: BTreeMap::new(),
        }
    }
}

fn default_modules() -> Vec<String> {
    vec!["idle".to_owned()]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            modules: default_modules(),
            module: BTreeMap::new(),
            bubbles: BubblesConfig::default(),
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
        toml::from_str(text).map_err(|error| ConfigError::invalid(path, error.to_string()))
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
    let theme: Theme =
        toml::from_str(text).map_err(|error| ConfigError::invalid(path, error.to_string()))?;
    check_theme(&theme).map_err(|message| ConfigError::invalid(path, message))?;
    Ok(theme)
}

/// Rejects values the UI can't use, naming the key.
fn check_theme(theme: &Theme) -> Result<(), String> {
    let motion = &theme.motion;
    if !(motion.spring.is_finite() && motion.spring > 0.0) {
        return Err(format!(
            "motion.spring must be above 0, got {}",
            motion.spring
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
        assert_eq!(Config::default().modules, ["idle"]);
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
                order: None,
                wide: None,
            }
        );
        config.check(AVAILABLE, path()).unwrap();

        let unknown = Config::parse("[bubbles.radio]\narea = \"left\"", path()).unwrap();
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

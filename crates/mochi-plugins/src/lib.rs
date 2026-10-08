//! Plugins: their manifest, the list in plugins.toml, the lock file that
//! pins what each one resolved to, and installing them.
//!
//! plugins.toml sits next to config.toml and lists plugins by id:
//!
//! ```toml
//! [plugins.pomodoro]
//! source = "git:github.com/User/mochi-pomodoro:main"
//! ```
//!
//! `mochi plugins install` fetches and builds them into
//! `$XDG_DATA_HOME/mochi/plugins/<id>/` and records the exact commit or
//! release in plugins.lock. mochid only reads: it never fetches or builds.

pub mod install;
pub mod manifest;
pub mod source;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use manifest::{Manifest, ManifestError};
pub use source::{Source, SourceError};

/// The modules compiled into mochid. A plugin can't take one of their ids.
pub const BUILTIN: [&str; 27] = [
    "idle",
    "osd",
    "workspaces",
    "media",
    "audio",
    "notifications",
    "launcher",
    "hub",
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
    "performance",
    "widgets",
    "notes",
    "emoji",
    "colors",
    "settings",
    "tour",
    "demo",
];

/// Where plugin files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locations {
    /// plugins.toml.
    pub list: PathBuf,
    /// plugins.lock, next to it.
    pub lock: PathBuf,
    /// Installed plugins, one directory each.
    pub installs: PathBuf,
}

impl Locations {
    /// The files next to `config_file`, and installs in
    /// `$XDG_DATA_HOME/mochi/plugins`.
    pub fn beside(config_file: &Path) -> Self {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("/nonexistent"));
        Self {
            list: config_file.with_file_name("plugins.toml"),
            lock: config_file.with_file_name("plugins.lock"),
            installs: data_home.join("mochi").join("plugins"),
        }
    }

    /// The garbage collector root keeping a plugin's `nix build` alive.
    pub fn nix_root(&self, id: &str) -> PathBuf {
        self.installs.join(".nix").join(id)
    }

    /// Where a plugin's files are: in place for `path:`, installed
    /// otherwise.
    pub fn dir(&self, id: &str, source: &Source) -> PathBuf {
        let beside = self.list.parent().unwrap_or(Path::new("."));
        source
            .path(beside)
            .unwrap_or_else(|| self.installs.join(id))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error("cannot read {path}: {error}")]
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("{path}: {message}")]
    Invalid { path: PathBuf, message: String },
    #[error("cannot write {path}: {error}")]
    Write {
        path: PathBuf,
        error: std::io::Error,
    },
}

/// Whether `program` is on the PATH, or is a path that exists.
pub fn on_path(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// plugins.toml.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginList {
    pub plugins: BTreeMap<String, Source>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListFile {
    #[serde(default)]
    plugins: BTreeMap<String, ListEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListEntry {
    source: String,
}

impl PluginList {
    /// Reads plugins.toml; a missing file lists nothing.
    pub fn load(path: &Path) -> Result<Self, ListError> {
        let Some(text) = read_optional(path)? else {
            return Ok(Self::default());
        };
        Self::parse(&text).map_err(|message| ListError::Invalid {
            path: path.to_owned(),
            message,
        })
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let file: ListFile = toml::from_str(text).map_err(|error| error.to_string())?;
        let mut plugins = BTreeMap::new();
        for (id, entry) in file.plugins {
            manifest::check_id(&id)?;
            if BUILTIN.contains(&id.as_str()) {
                return Err(format!("{id:?} is a builtin module, pick another id"));
            }
            let source = entry
                .source
                .parse()
                .map_err(|error: SourceError| error.to_string())?;
            plugins.insert(id, source);
        }
        Ok(Self { plugins })
    }
}

/// plugins.lock: what each plugin resolved to when it was installed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    #[serde(default)]
    pub plugins: BTreeMap<String, Locked>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locked {
    /// The source as plugins.toml had it.
    pub source: String,
    /// The plugin's version from its manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// For `git:`, the commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// For `git-release:`, the tag, the asset's URL and its BLAKE3 hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blake3: Option<String>,
}

impl Locked {
    /// The commit or tag, short, for showing.
    pub fn revision(&self) -> Option<String> {
        self.commit
            .as_ref()
            .map(|commit| commit.chars().take(10).collect())
            .or_else(|| self.tag.clone())
    }
}

const LOCK_HEADER: &str = "# Written by `mochi plugins`: what each plugin in plugins.toml resolved to.\n# `mochi plugins install` installs exactly this; `mochi plugins update` moves it.\n\n";

impl Lock {
    pub fn load(path: &Path) -> Result<Self, ListError> {
        let Some(text) = read_optional(path)? else {
            return Ok(Self::default());
        };
        toml::from_str(&text).map_err(|error| ListError::Invalid {
            path: path.to_owned(),
            message: error.to_string(),
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), ListError> {
        let write = |error| ListError::Write {
            path: path.to_owned(),
            error,
        };
        let text = toml::to_string(self).expect("the lock serializes");
        let temporary = path.with_extension("lock.tmp");
        std::fs::write(&temporary, format!("{LOCK_HEADER}{text}")).map_err(write)?;
        std::fs::rename(&temporary, path).map_err(write)
    }
}

fn read_optional(path: &Path) -> Result<Option<String>, ListError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ListError::Read {
            path: path.to_owned(),
            error,
        }),
    }
}

/// A plugin from plugins.toml and what its directory holds.
#[derive(Debug)]
pub struct Found {
    pub id: String,
    pub source: Source,
    pub dir: PathBuf,
    /// Why it can't load, when it can't: not installed, or a bad manifest.
    pub manifest: Result<Manifest, String>,
}

/// Every plugin plugins.toml lists, with its manifest when it's there.
pub fn discover(locations: &Locations) -> Result<Vec<Found>, ListError> {
    let list = PluginList::load(&locations.list)?;
    Ok(list
        .plugins
        .into_iter()
        .map(|(id, source)| {
            let dir = locations.dir(&id, &source);
            let manifest = match Manifest::load(&dir) {
                Ok(manifest) if manifest.plugin.id != id => Err(format!(
                    "{} says its id is {:?}, plugins.toml calls it {id:?}",
                    dir.join(manifest::FILE).display(),
                    manifest.plugin.id
                )),
                Ok(manifest) => Ok(manifest),
                Err(ManifestError::Missing(_)) if !matches!(source, Source::Path(_)) => {
                    Err(format!("not installed: run `mochi plugins install {id}`"))
                }
                Err(error) => Err(error.to_string()),
            };
            Found {
                id,
                source,
                dir,
                manifest,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_parses() {
        let list = PluginList::parse(
            "[plugins.pomodoro]\nsource = \"git:github.com/User/pomodoro:main\"\n\n[plugins.mine]\nsource = \"path:~/code/mine\"\n",
        )
        .unwrap();
        assert_eq!(list.plugins.len(), 2);
        assert!(matches!(list.plugins["mine"], Source::Path(_)));
    }

    #[test]
    fn the_list_refuses_builtin_ids_and_typos() {
        assert!(
            PluginList::parse("[plugins.media]\nsource = \"path:x\"\n")
                .unwrap_err()
                .contains("builtin")
        );
        assert!(
            PluginList::parse("[plugins.x]\nsorce = \"path:x\"\n")
                .unwrap_err()
                .contains("sorce")
        );
        assert!(
            PluginList::parse("[plugins.x]\nsource = \"x\"\n")
                .unwrap_err()
                .contains("git:")
        );
    }

    #[test]
    fn the_lock_round_trips() {
        let dir = std::env::temp_dir().join(format!("mochi-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plugins.lock");
        let mut lock = Lock::default();
        lock.plugins.insert(
            "pomodoro".into(),
            Locked {
                source: "git:github.com/User/pomodoro:main".into(),
                version: Some("0.1.0".into()),
                commit: Some("0123456789abcdef".into()),
                ..Locked::default()
            },
        );
        lock.save(&path).unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("# Written")
        );
        assert_eq!(Lock::load(&path).unwrap(), lock);
        assert_eq!(lock.plugins["pomodoro"].revision().unwrap(), "0123456789");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn discover_reports_missing_plugins() {
        let dir = std::env::temp_dir().join(format!("mochi-discover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mine")).unwrap();
        std::fs::write(
            dir.join("plugins.toml"),
            "[plugins.gone]\nsource = \"git:github.com/User/gone\"\n[plugins.mine]\nsource = \"path:mine\"\n[plugins.other]\nsource = \"path:mine\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("mine").join(manifest::FILE),
            "[plugin]\nid = \"mine\"\nname = \"Mine\"\nversion = \"1\"\napi = 1\n",
        )
        .unwrap();
        let locations = Locations {
            list: dir.join("plugins.toml"),
            lock: dir.join("plugins.lock"),
            installs: dir.join("installs"),
        };
        let found = discover(&locations).unwrap();
        let by_id = |id: &str| found.iter().find(|found| found.id == id).unwrap();
        assert!(
            by_id("gone")
                .manifest
                .as_ref()
                .unwrap_err()
                .contains("mochi plugins install gone")
        );
        assert!(by_id("mine").manifest.is_ok());
        assert!(
            by_id("other")
                .manifest
                .as_ref()
                .unwrap_err()
                .contains("calls it \"other\"")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

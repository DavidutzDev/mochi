//! `bento.toml`, next to config.toml: what `mochi bento` installed, and
//! where from. Bento writes it, never home-manager, so installing works
//! while home-manager owns config.toml and plugins.toml.
//!
//! ```toml
//! [plugins.pomodoro]
//! source = "git:github.com/User/mochi-pomodoro:main"
//! by = "cozy"
//!
//! [themes.mint]
//! source = "git:github.com/User/mochi-mint"
//! revision = "4f1c2a9d0e7b..."
//! version = "1.0.0"
//!
//! [bentos.cozy]
//! source = "git:github.com/User/cozy"
//! revision = "9e0d1c2b3a4f..."
//! version = "1.2.0"
//! ```
//!
//! Its plugins count as if plugins.toml listed them; plugins.toml wins
//! when both list an id.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{ListError, read_optional};

/// Its name, next to config.toml.
pub const FILE: &str = "bento.toml";

const HEADER: &str = "# Written by `mochi bento`: what it installed, and where from.\n# `mochi bento remove <id>` takes one out again.\n\n";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installed {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugins: BTreeMap<String, Entry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub themes: BTreeMap<String, Entry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bentos: BTreeMap<String, Entry>,
    /// Registries besides the default one, by name, with their index's URL;
    /// `bento` here points the default one elsewhere.
    /// The bento in use, when one is: its settings and widgets are the ones
    /// in place, and yours wait in Bento's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub registries: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Where it came from, as `mochi bento add` was given it.
    pub source: String,
    /// The commit it was installed at, for a git source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Its version, from its manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The bento that brought it, when one did: removing the bento
    /// removes it too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

impl Installed {
    /// `bento.toml` next to `config_file`.
    pub fn path(config_file: &Path) -> PathBuf {
        config_file.with_file_name(FILE)
    }

    /// Reads the file; a missing one has nothing installed.
    pub fn load(path: &Path) -> Result<Self, ListError> {
        let Some(text) = read_optional(path)? else {
            return Ok(Self::default());
        };
        toml::from_str(&text).map_err(|error| ListError::Invalid {
            path: path.to_owned(),
            message: error.to_string(),
        })
    }

    /// Writes the file, or removes it when nothing is installed.
    pub fn save(&self, path: &Path) -> Result<(), ListError> {
        let write = |error| ListError::Write {
            path: path.to_owned(),
            error,
        };
        if *self == Self::default() {
            return match std::fs::remove_file(path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(write(error)),
                _ => Ok(()),
            };
        }
        let text = toml::to_string(self).expect("bento.toml serializes");
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, format!("{HEADER}{text}")).map_err(write)?;
        std::fs::rename(&temporary, path).map_err(write)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_round_trips_and_goes_when_empty() {
        let dir = std::env::temp_dir().join(format!("mochi-bento-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);
        let mut installed = Installed::default();
        installed.plugins.insert(
            "pomodoro".into(),
            Entry {
                source: "git:github.com/User/mochi-pomodoro".into(),
                by: Some("cozy".into()),
                ..Entry::default()
            },
        );
        installed.save(&path).unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("by = \"cozy\"")
        );
        assert_eq!(Installed::load(&path).unwrap(), installed);
        Installed::default().save(&path).unwrap();
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

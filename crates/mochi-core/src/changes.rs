//! What the settings panel changed: `changes.toml` next to `config.toml`,
//! laid over `config.toml` and `theme.toml` when the daemon reads them.
//!
//! ```toml
//! [config.module.osd]
//! timeout_ms = 2000
//!
//! [theme.colors]
//! accent = "#30d158"
//! ```
//!
//! The panel never writes `config.toml` or `theme.toml`, which home-manager
//! keeps read-only. A change goes away by itself once the files say the
//! same, so pasting what the panel copied into them leaves nothing behind.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::{fmt, fs};

use toml::{Table, Value};

use crate::config::ConfigError;

/// Its name, next to `config.toml`.
pub const FILE: &str = "changes.toml";

/// The two files changes go over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum File {
    Config,
    Theme,
}

impl File {
    pub const ALL: [Self; 2] = [Self::Config, Self::Theme];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Theme => "theme",
        }
    }

    /// Splits `theme.colors.accent` into the file and the path in it.
    pub fn split(path: &str) -> Option<(Self, Vec<&str>)> {
        let mut parts = path.split('.');
        let file = match parts.next()? {
            "config" => Self::Config,
            "theme" => Self::Theme,
            _ => return None,
        };
        let rest: Vec<&str> = parts.collect();
        rest.iter()
            .all(|part| !part.is_empty())
            .then_some((file, rest))
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changes {
    pub config: Table,
    pub theme: Table,
}

impl Changes {
    /// `changes.toml` next to `config_file`.
    pub fn path(config_file: &Path) -> PathBuf {
        config_file.with_file_name(FILE)
    }

    /// Reads the file, or returns no changes when there is none.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let mut table: Table =
            toml::from_str(&text).map_err(|error| ConfigError::invalid(path, error.to_string()))?;
        let mut take = |name: &str| match table.remove(name) {
            None => Ok(Table::new()),
            Some(Value::Table(inner)) => Ok(inner),
            Some(_) => Err(ConfigError::invalid(
                path,
                format!("`{name}` must be a table"),
            )),
        };
        let changes = Self {
            config: take("config")?,
            theme: take("theme")?,
        };
        if let Some(key) = table.keys().next() {
            return Err(ConfigError::invalid(
                path,
                format!("unknown section `{key}`: only `config` and `theme`"),
            ));
        }
        Ok(changes)
    }

    /// Writes the file, or removes it when there are no changes.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if self.is_empty() {
            return match fs::remove_file(path) {
                Err(error) if error.kind() != ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            };
        }
        let mut table = Table::new();
        for file in File::ALL {
            if !self.table(file).is_empty() {
                table.insert(file.to_string(), Value::Table(self.table(file).clone()));
            }
        }
        let text = format!(
            "# What Mochi's settings panel changed, over config.toml and theme.toml.\n\
             # A change goes away once those files say the same.\n\n{}",
            toml::to_string_pretty(&table).map_err(io::Error::other)?
        );
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temporary = path.with_extension("toml.new");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)
    }

    pub fn is_empty(&self) -> bool {
        self.config.is_empty() && self.theme.is_empty()
    }

    pub fn table(&self, file: File) -> &Table {
        match file {
            File::Config => &self.config,
            File::Theme => &self.theme,
        }
    }

    pub fn table_mut(&mut self, file: File) -> &mut Table {
        match file {
            File::Config => &mut self.config,
            File::Theme => &mut self.theme,
        }
    }
}

/// Lays `over` on `base`: tables merge key by key, anything else replaces
/// what was there, lists included.
pub fn merge(base: &mut Table, over: &Table) {
    for (key, value) in over {
        match (base.get_mut(key), value) {
            (Some(Value::Table(inner)), Value::Table(over)) => merge(inner, over),
            _ => {
                base.insert(key.clone(), value.clone());
            }
        }
    }
}

/// `base` with `over` laid on it.
pub fn merged(base: &Table, over: &Table) -> Table {
    let mut table = base.clone();
    merge(&mut table, over);
    table
}

pub fn get<'a>(table: &'a Table, path: &[&str]) -> Option<&'a Value> {
    let (last, parents) = path.split_last()?;
    let mut current = table;
    for part in parents {
        current = current.get(*part)?.as_table()?;
    }
    current.get(*last)
}

/// Sets the value at `path`, making the tables on the way. A value that
/// isn't a table on the way is replaced by one.
pub fn set(table: &mut Table, path: &[&str], value: Value) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = table;
    for part in parents {
        let entry = current
            .entry((*part).to_owned())
            .or_insert_with(|| Value::Table(Table::new()));
        if !entry.is_table() {
            *entry = Value::Table(Table::new());
        }
        current = entry.as_table_mut().expect("made a table above");
    }
    current.insert((*last).to_owned(), value);
}

/// Removes the value at `path`, and the tables it leaves empty.
pub fn remove(table: &mut Table, path: &[&str]) {
    let Some((first, rest)) = path.split_first() else {
        return;
    };
    if rest.is_empty() {
        table.remove(*first);
        return;
    }
    if let Some(Value::Table(inner)) = table.get_mut(*first) {
        remove(inner, rest);
        if inner.is_empty() {
            table.remove(*first);
        }
    }
}

/// Drops the changes the files already say, and the ones equal to the
/// default where the files say nothing: they change nothing any more.
pub fn prune(changes: &mut Table, base: &Table, defaults: &Table) {
    let mut done = Vec::new();
    leaves(changes, &mut Vec::new(), &mut |path, value| {
        let parts: Vec<&str> = path.iter().map(String::as_str).collect();
        let reference = get(base, &parts).or_else(|| get(defaults, &parts));
        if reference == Some(value) {
            done.push(path.to_vec());
        }
    });
    for path in done {
        let parts: Vec<&str> = path.iter().map(String::as_str).collect();
        remove(changes, &parts);
    }
}

/// What `table` sets that isn't the default: what a user would write in
/// their files to get it.
pub fn diff(table: &Table, defaults: &Table) -> Table {
    let mut out = Table::new();
    for (key, value) in table {
        match (value, defaults.get(key)) {
            (Value::Table(inner), Some(Value::Table(default))) => {
                let inner = diff(inner, default);
                if !inner.is_empty() {
                    out.insert(key.clone(), Value::Table(inner));
                }
            }
            (value, default) if default == Some(value) => {}
            (value, _) => {
                out.insert(key.clone(), value.clone());
            }
        }
    }
    out
}

/// Calls `visit` with the path and value of everything in `table` that
/// isn't a table.
fn leaves(table: &Table, path: &mut Vec<String>, visit: &mut impl FnMut(&[String], &Value)) {
    for (key, value) in table {
        path.push(key.clone());
        match value {
            Value::Table(inner) => leaves(inner, path, visit),
            _ => visit(path, value),
        }
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> Table {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn merging_replaces_values_and_merges_tables() {
        let mut base = table(
            r#"
            modules = ["idle", "osd"]
            [module.osd]
            timeout_ms = 1500
            volume = false
            "#,
        );
        merge(
            &mut base,
            &table(
                r#"
                modules = ["idle"]
                [module.osd]
                timeout_ms = 2000
                "#,
            ),
        );
        assert_eq!(
            base,
            table(
                r#"
                modules = ["idle"]
                [module.osd]
                timeout_ms = 2000
                volume = false
                "#
            )
        );
    }

    #[test]
    fn set_and_remove_walk_paths() {
        let mut changes = Table::new();
        set(
            &mut changes,
            &["module", "osd", "timeout_ms"],
            Value::Integer(2000),
        );
        assert_eq!(
            get(&changes, &["module", "osd", "timeout_ms"]),
            Some(&Value::Integer(2000))
        );
        remove(&mut changes, &["module", "osd", "timeout_ms"]);
        assert!(changes.is_empty(), "{changes:?}");
    }

    #[test]
    fn pruning_drops_what_the_files_or_the_defaults_say() {
        let base = table(
            r#"
            [island]
            notices = "focus"
            [module.audio]
            max_volume = 200
            "#,
        );
        let defaults = table(
            r#"
            [island]
            notices = "all"
            panels = "focus"
            [module.audio]
            max_volume = 100
            "#,
        );
        let mut changes = table(
            r#"
            [island]
            notices = "focus"   # the file says so now
            panels = "focus"    # the default, and the file says nothing
            [module.audio]
            max_volume = 100    # the default, but the file says 200
            "#,
        );
        prune(&mut changes, &base, &defaults);
        assert_eq!(changes, table("[module.audio]\nmax_volume = 100"));
    }

    #[test]
    fn the_diff_keeps_what_isnt_a_default() {
        let defaults = table(
            r#"
            modules = ["idle"]
            [island]
            notices = "all"
            panels = "focus"
            "#,
        );
        let effective = table(
            r#"
            modules = ["idle", "osd"]
            [island]
            notices = "focus"
            panels = "focus"
            [module.osd]
            timeout_ms = 2000
            "#,
        );
        assert_eq!(
            diff(&effective, &defaults),
            table(
                r#"
                modules = ["idle", "osd"]
                [island]
                notices = "focus"
                [module.osd]
                timeout_ms = 2000
                "#
            )
        );
    }

    #[test]
    fn paths_name_their_file() {
        assert_eq!(
            File::split("theme.colors.accent"),
            Some((File::Theme, vec!["colors", "accent"]))
        );
        assert_eq!(File::split("config"), Some((File::Config, vec![])));
        assert_eq!(File::split("colors.accent"), None);
        assert_eq!(File::split("theme..accent"), None);
    }

    #[test]
    fn the_file_round_trips_and_goes_when_empty() {
        let dir = std::env::temp_dir().join(format!("mochi-changes-{}", std::process::id()));
        let path = dir.join(FILE);
        let changes = Changes {
            config: table("[module.osd]\ntimeout_ms = 2000"),
            theme: table("[colors]\naccent = \"#30d158\""),
        };
        changes.save(&path).unwrap();
        assert_eq!(Changes::load(&path).unwrap(), changes);

        Changes::default().save(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(Changes::load(&path).unwrap(), Changes::default());

        fs::write(&path, "[colours]\naccent = 1").unwrap();
        let error = Changes::load(&path).unwrap_err().to_string();
        assert!(error.contains("colours"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }
}

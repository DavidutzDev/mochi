//! `mochi-bento.toml`: a whole setup to share. Its settings go over
//! `config.toml` and `theme.toml`, its widgets replace `widgets.toml`, and
//! it names the plugins it needs. Themes it brings sit next to it in
//! `themes/<id>/mochi-theme.toml`.
//!
//! ```toml
//! [bento]
//! id = "cozy"
//! name = "Cozy"
//! version = "1.0.0"
//! mochi = "0.0.8"
//!
//! [plugins]
//! pomodoro = "git:github.com/User/mochi-pomodoro"
//!
//! [config]
//! modules = ["idle", "osd", "control-center", "widgets", "pomodoro"]
//!
//! [theme]
//! preset = "cozy"
//!
//! [[widget]]
//! id = "w1"
//! module = "widgets"
//! widget = "clock"
//! output = "screen-1"
//! width = 14
//! height = 7
//! ```

use std::collections::BTreeMap;
use std::path::{Component, Path};

use mochi_core::toml::Table;
use mochi_module_widgets::layout::Placed;
use mochi_plugins::Source;
use serde::{Deserialize, Serialize};

use super::screens;

pub const FILE: &str = "mochi-bento.toml";

const HEADER: &str = "# A bento: a Mochi setup to share. `mochi bento add <where it is>` installs\n# it, and the Bento page of Mochi's documentation explains this file.\n\n";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bento {
    pub bento: About,
    /// The plugins it needs, by id, with their source as plugins.toml takes
    /// it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugins: BTreeMap<String, String>,
    /// Laid over `config.toml`.
    #[serde(default, skip_serializing_if = "Table::is_empty")]
    pub config: Table,
    /// Laid over `theme.toml`.
    #[serde(default, skip_serializing_if = "Table::is_empty")]
    pub theme: Table,
    /// In place of `widgets.toml`, on screens named by size.
    #[serde(default, rename = "widget", skip_serializing_if = "Vec::is_empty")]
    pub widgets: Vec<Placed>,
}

/// `[bento]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct About {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The oldest Mochi it works with.
    pub mochi: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// Pictures of it, relative to the manifest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub screenshots: Vec<String>,
    /// An image to set as the wallpaper, relative to the manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper: Option<String>,
}

impl Bento {
    pub fn parse(text: &str) -> Result<Self, String> {
        let bento: Self = mochi_core::toml::from_str(text).map_err(|error| error.to_string())?;
        bento.check()?;
        Ok(bento)
    }

    /// The manifest in `dir`.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join(FILE);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Self::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn to_toml(&self) -> String {
        let text = mochi_core::toml::to_string_pretty(self).expect("a bento serializes");
        format!("{HEADER}{text}")
    }

    /// The plugins' sources, parsed.
    pub fn sources(&self) -> Result<BTreeMap<String, Source>, String> {
        self.plugins
            .iter()
            .map(|(id, text)| {
                let source: Source = text
                    .parse()
                    .map_err(|error| format!("[plugins] {id}: {error}"))?;
                if matches!(source, Source::Path(_)) {
                    return Err(format!(
                        "[plugins] {id}: a bento can't bring a path: plugin, only git: and git-release: ones"
                    ));
                }
                Ok((id.clone(), source))
            })
            .collect()
    }

    fn check(&self) -> Result<(), String> {
        let about = &self.bento;
        mochi_plugins::manifest::check_id(&about.id).map_err(|error| format!("[bento] {error}"))?;
        mochi_core::version::parse(&about.mochi).ok_or_else(|| {
            format!(
                "[bento] `mochi = {:?}` isn't a version like \"0.0.8\"",
                about.mochi
            )
        })?;
        for id in self.plugins.keys() {
            mochi_plugins::manifest::check_id(id).map_err(|error| format!("[plugins] {error}"))?;
            if mochi_plugins::BUILTIN.contains(&id.as_str()) {
                return Err(format!("[plugins] {id:?} is a builtin module"));
            }
        }
        self.sources()?;
        for file in about.screenshots.iter().chain(&about.wallpaper) {
            inside(file).map_err(|error| format!("[bento] {error}"))?;
        }
        let mut ids = std::collections::BTreeSet::new();
        for widget in &self.widgets {
            if !ids.insert(&widget.id) {
                return Err(format!("two widgets have the id {:?}", widget.id));
            }
            if screens::parse_role(&widget.output).is_none() {
                return Err(format!(
                    "[[widget]] {}: `output` is a screen by size, like \"screen-1\" for the largest, not {:?}",
                    widget.id, widget.output
                ));
            }
        }
        Ok(())
    }
}

/// A path that stays inside the bento.
fn inside(file: &str) -> Result<(), String> {
    let path = Path::new(file);
    let fits = !file.is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if fits {
        Ok(())
    } else {
        Err(format!("{file:?} must be a path inside the bento"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COZY: &str = r#"
[bento]
id = "cozy"
name = "Cozy"
version = "1.0.0"
mochi = "0.0.1"
wallpaper = "wall.jpg"

[plugins]
pomodoro = "git:github.com/User/mochi-pomodoro"

[config]
modules = ["idle", "widgets", "pomodoro"]

[config.module.idle]
format = "%H:%M"

[theme]
preset = "nord"

[[widget]]
id = "w1"
module = "widgets"
widget = "clock"
output = "screen-1"
width = 14
height = 7
"#;

    #[test]
    fn a_bento_reads_and_writes_back() {
        let bento = Bento::parse(COZY).unwrap();
        assert_eq!(bento.widgets[0].output, "screen-1");
        assert_eq!(
            bento.config["module"]["idle"]["format"].as_str(),
            Some("%H:%M")
        );
        assert_eq!(Bento::parse(&bento.to_toml()).unwrap(), bento);
    }

    #[test]
    fn mistakes_are_named() {
        let error = |from: &str, to: &str| Bento::parse(&COZY.replace(from, to)).unwrap_err();
        assert!(error("\"screen-1\"", "\"DP-3\"").contains("screen by size"));
        assert!(error("wall.jpg", "../wall.jpg").contains("inside the bento"));
        assert!(error("wall.jpg", "/etc/wall.jpg").contains("inside the bento"));
        assert!(error("git:github.com/User/mochi-pomodoro", "path:~/x").contains("path: plugin"));
        assert!(error("pomodoro =", "idle =").contains("builtin"));
        assert!(error("mochi = \"0.0.1\"", "mochi = \"soon\"").contains("isn't a version"));
        assert!(error("[theme]", "[looks]").contains("unknown field"));
    }
}

//! `widgets.toml`: where each placed widget is. The editor rewrites it on
//! every change, and the module reads it again when it changes on disk.
//!
//! ```toml
//! layout = "Work"
//!
//! [[widget]]
//! id = "w1"
//! module = "widgets"
//! widget = "clock"
//! variant = "digital"
//! output = "DP-3"
//! anchor = "top-left"
//! x = 2
//! y = 3
//! width = 14
//! height = 8
//!
//! [widget.settings]
//! seconds = true
//! ```
//!
//! Positions and sizes are in grid cells. The widget's `anchor` point sits
//! at the same point of the screen, moved by `x` and `y` cells, so a widget
//! anchored bottom-right stays in that corner on any screen size.
//!
//! `variant` is the look it has, of those its widget offers; without one,
//! the first. `layout` names the saved layout this arrangement is, see
//! [`crate::saved`]: changes go to that one too.

use std::fmt::Write as _;
use std::path::Path;

use mochi_core::nix;
use serde::{Deserialize, Serialize};

pub const FILE: &str = "widgets.toml";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    pub const ALL: [&str; 9] = [
        "top-left",
        "top",
        "top-right",
        "left",
        "center",
        "right",
        "bottom-left",
        "bottom",
        "bottom-right",
    ];

    pub fn parse(text: &str) -> Option<Self> {
        Self::deserialize(toml::Value::String(text.to_owned())).ok()
    }

    pub fn as_str(self) -> &'static str {
        Self::ALL[self as usize]
    }
}

/// One placed widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placed {
    /// Its own: two clocks are two instances with their own settings.
    pub id: String,
    /// The module offering it, and which of its widgets.
    pub module: String,
    pub widget: String,
    /// Which of the widget's looks, for one that has several.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// The monitor's name, like `DP-3`.
    pub output: String,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Its layer: where widgets overlap, a higher one is on top.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub z: i32,
    #[serde(default, skip_serializing_if = "toml::Table::is_empty")]
    pub settings: toml::Table,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &i32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    /// The saved layout this is, which follows its changes.
    #[serde(default, rename = "layout", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, rename = "widget")]
    pub widgets: Vec<Placed>,
}

impl Layout {
    pub fn parse(text: &str) -> Result<Self, String> {
        let layout: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        let mut seen = std::collections::BTreeSet::new();
        for widget in &layout.widgets {
            if !seen.insert(&widget.id) {
                return Err(format!("two widgets have the id {:?}", widget.id));
            }
        }
        Ok(layout)
    }

    /// `None` when there is no file, which is an empty layout.
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text)
                .map(Some)
                .map_err(|error| format!("{}: {error}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("{}: {error}", path.display())),
        }
    }

    pub fn to_toml(&self) -> String {
        let body = toml::to_string_pretty(self).unwrap_or_default();
        format!(
            "# Mochi's widgets. `mochi ipc widgets edit` arranges them and rewrites\n\
             # this file; changes made here apply as soon as it's saved.\n\n{body}"
        )
    }

    /// Writes through a temporary file, so a crash or the watcher never
    /// sees half a file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, self.to_toml())?;
        std::fs::rename(temporary, path)
    }

    pub fn get_mut(&mut self, id: &str) -> Result<&mut Placed, String> {
        self.widgets
            .iter_mut()
            .find(|widget| widget.id == id)
            .ok_or_else(|| format!("no widget {id}"))
    }

    /// An id no widget has, here or in `others`: `w` and the lowest free
    /// number. The saved layouts are the others, since a module keeps a
    /// note's text by its widget's id.
    pub fn new_id<'a>(&'a self, others: impl IntoIterator<Item = &'a Layout>) -> String {
        let taken: std::collections::BTreeSet<&str> = std::iter::once(self)
            .chain(others)
            .flat_map(|layout| &layout.widgets)
            .map(|widget| widget.id.as_str())
            .collect();
        (1..)
            .map(|number| format!("w{number}"))
            .find(|id| !taken.contains(id.as_str()))
            .unwrap_or_default()
    }

    /// The layout as Nix, for `programs.mochi.widgets` in home-manager.
    pub fn to_nix(&self) -> String {
        let mut out = String::from("[\n");
        for widget in &self.widgets {
            out.push_str("  {\n");
            let fields: [(&str, String); 9] = [
                ("id", nix::string(&widget.id)),
                ("module", nix::string(&widget.module)),
                ("widget", nix::string(&widget.widget)),
                ("output", nix::string(&widget.output)),
                ("anchor", nix::string(widget.anchor.as_str())),
                ("x", widget.x.to_string()),
                ("y", widget.y.to_string()),
                ("width", widget.width.to_string()),
                ("height", widget.height.to_string()),
            ];
            for (key, value) in fields {
                let _ = writeln!(out, "    {key} = {value};");
            }
            if let Some(variant) = &widget.variant {
                let _ = writeln!(out, "    variant = {};", nix::string(variant));
            }
            if widget.z != 0 {
                let _ = writeln!(out, "    z = {};", widget.z);
            }
            if !widget.settings.is_empty() {
                out.push_str("    settings = {\n");
                for (key, value) in &widget.settings {
                    let _ = writeln!(out, "      {} = {};", nix::key(key), nix::value(value));
                }
                out.push_str("    };\n");
            }
            out.push_str("  }\n");
        }
        out.push(']');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
[[widget]]
id = "w1"
module = "widgets"
widget = "clock"
output = "DP-3"
anchor = "bottom-right"
x = -2
y = -3
width = 14
height = 8

[widget.settings]
seconds = true
timezone = "Europe/Paris"
"#;

    #[test]
    fn reads_and_writes_the_same_layout() {
        let layout = Layout::parse(EXAMPLE).unwrap();
        let widget = &layout.widgets[0];
        assert_eq!(widget.anchor, Anchor::BottomRight);
        assert_eq!((widget.x, widget.y), (-2, -3));
        assert_eq!(widget.settings["seconds"].as_bool(), Some(true));
        assert_eq!(Layout::parse(&layout.to_toml()).unwrap(), layout);
    }

    #[test]
    fn refuses_mistakes() {
        let typo = EXAMPLE.replace("anchor", "anchr");
        assert!(Layout::parse(&typo).unwrap_err().contains("anchr"));
        let corner = EXAMPLE.replace("bottom-right", "corner");
        assert!(Layout::parse(&corner).is_err());
        let twice = format!(
            "{EXAMPLE}\n{}",
            EXAMPLE
                .replace("[widget.settings]", "")
                .replace("seconds = true", "")
                .replace("timezone = \"Europe/Paris\"", "")
        );
        assert!(Layout::parse(&twice).unwrap_err().contains("two widgets"));
        assert_eq!(Layout::parse("").unwrap(), Layout::default());
    }

    #[test]
    fn new_ids_fill_gaps() {
        let mut layout = Layout::parse(EXAMPLE).unwrap();
        assert_eq!(layout.new_id([]), "w2");
        layout.widgets[0].id = "w2".into();
        assert_eq!(layout.new_id([]), "w1");
        // Not one a saved layout has.
        let saved = Layout::parse(EXAMPLE).unwrap();
        assert_eq!(layout.new_id([&saved]), "w3");
    }

    #[test]
    fn variants_and_names_are_optional() {
        // A file from before variants reads, and writes back the same.
        let old = Layout::parse(EXAMPLE).unwrap();
        assert_eq!(old.widgets[0].variant, None);
        assert_eq!(old.name, None);
        assert!(!old.to_toml().contains("variant"));
        assert!(!old.to_toml().contains("layout ="));

        let new = Layout::parse(&format!(
            "layout = \"Work\"\n{}",
            EXAMPLE.replace(
                "widget = \"clock\"",
                "widget = \"clock\"\nvariant = \"stacked\""
            )
        ))
        .unwrap();
        assert_eq!(new.name.as_deref(), Some("Work"));
        assert_eq!(new.widgets[0].variant.as_deref(), Some("stacked"));
        assert_eq!(Layout::parse(&new.to_toml()).unwrap(), new);
        assert!(new.to_nix().contains("variant = \"stacked\";"));
    }

    #[test]
    fn anchors_read_and_print() {
        for name in Anchor::ALL {
            assert_eq!(Anchor::parse(name).unwrap().as_str(), name);
        }
        assert_eq!(Anchor::parse("middle"), None);
    }

    #[test]
    fn exports_nix() {
        let mut layout = Layout::parse(EXAMPLE).unwrap();
        layout.widgets[0]
            .settings
            .insert("label".into(), toml::Value::String("say \"${hi}\"".into()));
        layout.widgets[0]
            .settings
            .insert("weird key".into(), toml::Value::Float(2.0));
        let nix = layout.to_nix();
        assert!(nix.contains("anchor = \"bottom-right\";"), "{nix}");
        assert!(nix.contains("x = -2;"), "{nix}");
        assert!(nix.contains("seconds = true;"), "{nix}");
        assert!(nix.contains(r#"label = "say \"\${hi}\"";"#), "{nix}");
        assert!(nix.contains(r#""weird key" = 2.0;"#), "{nix}");
    }

    #[test]
    fn saves_through_a_temporary_file() {
        let dir = std::env::temp_dir().join(format!("mochi-widgets-{}", std::process::id()));
        let path = dir.join(FILE);
        assert_eq!(Layout::load(&path).unwrap(), None);
        let layout = Layout::parse(EXAMPLE).unwrap();
        layout.save(&path).unwrap();
        assert_eq!(Layout::load(&path).unwrap(), Some(layout));
        assert!(!path.with_extension("toml.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

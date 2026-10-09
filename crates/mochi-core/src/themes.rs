//! Themes: a color for every role, in a dark and a light version, as
//! `mochi-theme.toml` says. Mochi brings its presets as themes; others
//! are installed in `$XDG_DATA_HOME/mochi/themes/<id>/`, and
//! `preset = "<id>"` in `theme.toml` picks either.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use mochi_protocol::Color;
use serde::Deserialize;
use toml::{Table, Value};

/// A theme's manifest, at the root of its directory.
pub const MANIFEST: &str = "mochi-theme.toml";

/// The roles a theme colors, in the order of `[colors]`.
pub const ROLES: [&str; 12] = [
    "background",
    "surface",
    "raised",
    "highlight",
    "foreground",
    "muted",
    "accent",
    "on_accent",
    "danger",
    "success",
    "border",
    "shadow",
];

/// The themes Mochi brings, in the order the settings list them.
const BUNDLED: [(&str, &str); 6] = [
    ("obsidian", include_str!("../themes/obsidian.toml")),
    ("catppuccin", include_str!("../themes/catppuccin.toml")),
    ("nord", include_str!("../themes/nord.toml")),
    ("gruvbox", include_str!("../themes/gruvbox.toml")),
    ("rose-pine", include_str!("../themes/rose-pine.toml")),
    ("tokyo-night", include_str!("../themes/tokyo-night.toml")),
];

/// `mochi-theme.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFile {
    pub theme: About,
    /// Role to color. A role left out takes Obsidian's.
    pub dark: Option<BTreeMap<String, String>>,
    pub light: Option<BTreeMap<String, String>>,
}

/// `[theme]`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct About {
    /// What `preset` names it by.
    pub id: String,
    pub name: String,
    pub version: String,
    /// The oldest Mochi it works with.
    pub mochi: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub homepage: Option<String>,
}

impl ThemeFile {
    /// Reads a manifest, checking its id and colors.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        check_id(&file.theme.id)?;
        crate::version::parse(&file.theme.mochi).ok_or_else(|| {
            format!(
                "`mochi = {:?}` isn't a version like \"0.0.8\"",
                file.theme.mochi
            )
        })?;
        if file.dark.is_none() && file.light.is_none() {
            return Err("a theme needs [dark], [light] or both".into());
        }
        for (part, palette) in [("dark", &file.dark), ("light", &file.light)] {
            for (role, color) in palette.iter().flatten() {
                if !ROLES.contains(&role.as_str()) {
                    return Err(format!(
                        "[{part}]: unknown role `{role}`, expected one of {}",
                        ROLES.join(", ")
                    ));
                }
                Color::try_from(color.clone())
                    .map_err(|error| format!("[{part}] {role}: {error}"))?;
            }
        }
        Ok(file)
    }

    /// Its colors as `[colors]` would set them: the light or dark version,
    /// or the one it has for both, and for the roles it leaves out
    /// Obsidian's of that version, so a dark-only theme stays dark.
    pub fn colors(&self, light: bool) -> Table {
        let (palette, light) = match (&self.dark, &self.light) {
            (_, Some(palette)) if light => (palette, true),
            (Some(palette), _) => (palette, false),
            (None, Some(palette)) => (palette, true),
            (None, None) => unreachable!("parse needs [dark] or [light]"),
        };
        let mut colors = if self.theme.id == "obsidian" {
            Table::new()
        } else {
            bundled("obsidian").colors(light)
        };
        for (role, color) in palette {
            colors.insert(role.clone(), Value::String(color.clone()));
        }
        colors
    }
}

/// Lowercase letters, digits, `-` and `_`, starting with a letter, and
/// not `wallpaper`, which makes a palette from the wallpaper.
pub fn check_id(id: &str) -> Result<(), String> {
    let mut chars = id.chars();
    let fits = chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || "-_".contains(char));
    if !fits {
        return Err(format!(
            "the id {id:?} must be lowercase letters, digits, `-` and `_`, starting with a letter"
        ));
    }
    if id == "wallpaper" {
        return Err("`wallpaper` is the preset made from the wallpaper".into());
    }
    Ok(())
}

/// Whether Mochi brings a theme by this id.
pub fn is_bundled(id: &str) -> bool {
    BUNDLED.iter().any(|(bundled, _)| *bundled == id)
}

/// The ids of the themes Mochi brings, in their order.
pub fn bundled_ids() -> impl Iterator<Item = &'static str> {
    BUNDLED.iter().map(|(id, _)| *id)
}

fn bundled(id: &str) -> ThemeFile {
    let (_, text) = BUNDLED
        .iter()
        .find(|(bundled, _)| *bundled == id)
        .expect("a bundled theme");
    ThemeFile::parse(text).expect("the bundled themes read")
}

/// Where installed themes are: `$XDG_DATA_HOME/mochi/themes`.
pub fn dir() -> Option<PathBuf> {
    crate::config::data_dir().map(|data| data.join("themes"))
}

/// The theme `preset` names: one Mochi brings, or one installed.
pub fn find(id: &str) -> Result<ThemeFile, String> {
    if is_bundled(id) {
        return Ok(bundled(id));
    }
    let unknown = || {
        let ids: Vec<String> = list().into_iter().map(|theme| theme.theme.id).collect();
        format!(
            "unknown preset {id:?}, expected one of {}, wallpaper, or a theme installed in {}",
            ids.join(", "),
            dir().map_or_else(
                || "~/.local/share/mochi/themes".into(),
                |dir| dir.display().to_string()
            )
        )
    };
    if check_id(id).is_err() {
        return Err(unknown());
    }
    let Some(dir) = dir() else {
        return Err(unknown());
    };
    match read(&dir.join(id)) {
        Err(error) if error.is_empty() => Err(unknown()),
        result => result,
    }
}

/// The theme in `dir`, checked against its directory's name and the
/// running Mochi. An empty error means there's none there.
pub fn read(dir: &Path) -> Result<ThemeFile, String> {
    let path = dir.join(MANIFEST);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Err(String::new()),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let file = ThemeFile::parse(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let name = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");
    if file.theme.id != name {
        return Err(format!(
            "{}: its id is {:?}, but it's installed as {name:?}",
            path.display(),
            file.theme.id
        ));
    }
    crate::version::supports(&file.theme.mochi)
        .map_err(|error| format!("the theme {name:?}: {error}"))?;
    Ok(file)
}

/// Every theme: Mochi's first, in their order, then the installed ones by
/// id. An installed theme that doesn't read is logged and left out.
pub fn list() -> Vec<ThemeFile> {
    let mut themes: Vec<ThemeFile> = BUNDLED.iter().map(|(id, _)| bundled(id)).collect();
    let Some(entries) = dir().and_then(|dir| std::fs::read_dir(dir).ok()) else {
        return themes;
    };
    let mut installed = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || is_bundled(&name) || !entry.path().is_dir() {
            continue;
        }
        match read(&entry.path()) {
            Ok(theme) => installed.push(theme),
            Err(error) if error.is_empty() => {}
            Err(error) => tracing::warn!("{error}"),
        }
    }
    installed.sort_by(|a, b| a.theme.id.cmp(&b.theme.id));
    themes.extend(installed);
    themes
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r##"
[theme]
id = "mint"
name = "Mint"
version = "1.0.0"
mochi = "0.0.1"

[dark]
accent = "#3eb489"
"##;

    #[test]
    fn mochi_brings_its_presets_as_themes() {
        let ids: Vec<String> = list().into_iter().map(|theme| theme.theme.id).collect();
        assert_eq!(
            ids[..6],
            [
                "obsidian",
                "catppuccin",
                "nord",
                "gruvbox",
                "rose-pine",
                "tokyo-night"
            ]
        );
        for (id, _) in BUNDLED {
            let theme = find(id).unwrap();
            assert_eq!(theme.colors(false).len(), 12, "{id}");
            assert_eq!(theme.colors(true).len(), 12, "{id}");
        }
    }

    #[test]
    fn a_theme_may_leave_out_roles_and_a_version() {
        let theme = ThemeFile::parse(SMALL).unwrap();
        let dark = theme.colors(false);
        assert_eq!(dark["accent"].as_str(), Some("#3eb489"));
        assert_eq!(dark["surface"].as_str(), Some("#1c1c1e"));
        // A dark-only theme stays dark when the light version is asked for.
        let light = theme.colors(true);
        assert_eq!(light["accent"].as_str(), Some("#3eb489"));
        assert_eq!(light["surface"].as_str(), Some("#1c1c1e"));
    }

    #[test]
    fn mistakes_are_named() {
        let error = |text: &str| ThemeFile::parse(text).unwrap_err();
        assert!(error(&SMALL.replace("accent", "accnet")).contains("unknown role `accnet`"));
        assert!(error(&SMALL.replace("#3eb489", "mint")).contains("[dark] accent"));
        assert!(error(&SMALL.replace("\"mint\"", "\"Mint\"")).contains("lowercase"));
        assert!(error(&SMALL.replace("0.0.1", "soon")).contains("isn't a version"));
        assert!(
            error(&SMALL.replace("[dark]\naccent = \"#3eb489\"", "")).contains("[dark], [light]")
        );
        assert!(error(&format!("{SMALL}\nfont = \"x\"")).contains("unknown role `font`"));
        assert!(error(&format!("{SMALL}\n[fonts]\nbody = \"x\"")).contains("unknown field"));
    }

    #[test]
    fn installed_themes_are_read_from_their_directory() {
        let dir = std::env::temp_dir().join(format!("mochi-themes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mint")).unwrap();
        std::fs::write(dir.join("mint").join(MANIFEST), SMALL).unwrap();
        assert_eq!(read(&dir.join("mint")).unwrap().theme.name, "Mint");
        assert_eq!(read(&dir.join("none")), Err(String::new()));
        std::fs::create_dir_all(dir.join("other")).unwrap();
        std::fs::write(dir.join("other").join(MANIFEST), SMALL).unwrap();
        assert!(
            read(&dir.join("other"))
                .unwrap_err()
                .contains("installed as \"other\"")
        );
        std::fs::write(
            dir.join("mint").join(MANIFEST),
            SMALL.replace("0.0.1", "999.0"),
        )
        .unwrap();
        assert!(
            read(&dir.join("mint"))
                .unwrap_err()
                .contains("needs Mochi 999.0")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

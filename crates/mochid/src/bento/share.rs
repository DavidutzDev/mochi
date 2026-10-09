//! `mochi bento share`: a bento from the setup running now. It takes what
//! isn't a default from the files and the settings panel's changes, the
//! theme, the widgets and the plugins, and leaves out what belongs to this
//! machine or this person, saying what.

use std::path::{Path, PathBuf};

use mochi_core::toml::{Table, Value};
use mochi_module_widgets::layout::Layout;
use mochi_plugins::{Locations, PluginList, Source};

use super::manifest::{About, Bento, FILE};
use super::screens;
use crate::settings::Store;

/// Where an option's values come from, for the ones that name this
/// machine's things.
const MACHINE_SOURCES: [&str; 1] = ["audio-device"];

/// Keys that hold where someone is.
const PLACE_KEYS: [&str; 4] = ["latitude", "longitude", "location", "city"];

/// Keys that hold a secret.
const SECRET_KEYS: [&str; 8] = [
    "token",
    "password",
    "secret",
    "api_key",
    "apikey",
    "key",
    "auth",
    "credentials",
];

#[derive(Debug, Default)]
pub struct Options {
    pub name: Option<String>,
    /// Bring the wallpaper along.
    pub wallpaper: bool,
    /// Write into a directory that isn't empty.
    pub force: bool,
    /// Print the manifest instead of writing a directory.
    pub print: bool,
}

/// What was left out, and why, for the user to check.
type Left = Vec<String>;

pub fn share(config_file: &Path, dir: &Path, options: &Options) -> Result<(), String> {
    let id = id_from(dir)?;
    if !options.print {
        let busy = std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some());
        if busy && !options.force {
            return Err(format!(
                "{} isn't empty; pick another directory, or add --force to write into it",
                dir.display()
            ));
        }
    }
    let (store, _) = Store::load(config_file, None).map_err(|error| error.to_string())?;
    let (mut config, mut theme) = store.exported();
    let mut left = Left::new();

    // Settings that name this machine's things, or the person.
    let mut paths = Vec::new();
    leaves(&config, &mut Vec::new(), &mut paths);
    for path in paths {
        let dotted = format!("config.{}", path.join("."));
        let machine = store
            .source_of(&dotted)
            .is_some_and(|source| MACHINE_SOURCES.contains(&source));
        if machine {
            remove(&mut config, &path);
            left.push(format!("{dotted}: names a device of this machine"));
        }
    }
    clean(&mut config, "config", &mut left);
    clean(&mut theme, "theme", &mut left);

    // The plugins the shared modules use.
    let locations = Locations::beside(config_file);
    let listed = PluginList::of(&locations).map_err(|error| error.to_string())?;
    let enabled: Vec<String> = config
        .get("modules")
        .and_then(Value::as_array)
        .map(|modules| {
            modules
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let mut plugins = std::collections::BTreeMap::new();
    let mut dropped = Vec::new();
    for (plugin, source) in &listed.plugins {
        if !enabled.contains(plugin) {
            continue;
        }
        if matches!(source, Source::Path(_)) {
            left.push(format!(
                "the plugin {plugin}: it's a local directory; publish it to share it"
            ));
            dropped.push(plugin.clone());
        } else {
            plugins.insert(plugin.clone(), source.to_string());
        }
    }
    if !dropped.is_empty() {
        if let Some(Value::Array(modules)) = config.get_mut("modules") {
            modules.retain(|module| {
                !module
                    .as_str()
                    .is_some_and(|id| dropped.iter().any(|d| d == id))
            });
        }
        if let Some(Value::Table(sections)) = config.get_mut("module") {
            for plugin in &dropped {
                sections.remove(plugin);
            }
        }
    }

    // A theme that isn't one Mochi brings travels with the bento.
    let preset = theme
        .get("preset")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut themes = Vec::new();
    if let Some(preset) = &preset
        && preset != "wallpaper"
        && !mochi_core::themes::is_bundled(preset)
    {
        match mochi_core::themes::dir().map(|dir| dir.join(preset)) {
            Some(theme_dir) if theme_dir.join(mochi_core::themes::MANIFEST).is_file() => {
                themes.push((preset.clone(), theme_dir));
            }
            _ => {
                theme.remove("preset");
                left.push(format!("the theme {preset}: it isn't installed"));
            }
        }
    }

    // Widgets, on screens named by size.
    let widgets_file = config_file.with_file_name(mochi_module_widgets::FILE);
    let mut widgets = Vec::new();
    if let Some(layout) = Layout::load(&widgets_file)? {
        let screens = screens::connected();
        let shared =
            |module: &str| mochi_plugins::BUILTIN.contains(&module) || plugins.contains_key(module);
        for mut widget in layout.widgets {
            let Some(index) = screens.iter().position(|name| *name == widget.output) else {
                left.push(format!(
                    "the widget {} ({}): its screen {} isn't connected",
                    widget.id, widget.widget, widget.output
                ));
                continue;
            };
            if !shared(&widget.module) {
                left.push(format!(
                    "the widget {} ({}): its plugin {} isn't shared",
                    widget.id, widget.widget, widget.module
                ));
                continue;
            }
            widget.output = screens::role(index);
            clean(
                &mut widget.settings,
                &format!("widget {}", widget.id),
                &mut left,
            );
            widgets.push(widget);
        }
    }

    // The wallpaper, when asked.
    let mut wallpaper = None;
    if options.wallpaper {
        match mochi_core::palette::source("auto") {
            Ok(mochi_core::palette::Source::Image(path)) if !options.print => {
                let extension = path
                    .extension()
                    .map(|extension| extension.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "png".into());
                wallpaper = Some((format!("wallpaper.{extension}"), path));
            }
            Ok(mochi_core::palette::Source::Image(_)) => {
                left.push("the wallpaper: a printed bento is one file".into());
            }
            Ok(mochi_core::palette::Source::Color(_)) => {
                left.push("the wallpaper: it's a plain color".into());
            }
            Err(error) => left.push(format!("the wallpaper: {error}")),
        }
    }

    let bento = Bento {
        bento: About {
            id: id.clone(),
            name: options.name.clone().unwrap_or_else(|| title(&id)),
            version: "1.0.0".into(),
            mochi: mochi_core::version::VERSION.into(),
            description: String::new(),
            authors: Vec::new(),
            homepage: None,
            screenshots: Vec::new(),
            wallpaper: wallpaper.as_ref().map(|(name, _)| name.clone()),
        },
        plugins,
        config,
        theme,
        widgets,
    };

    if options.print {
        if !themes.is_empty() {
            left.push(format!(
                "the theme {}: a printed bento is one file; share a directory to bring it",
                themes[0].0
            ));
        }
        print!("{}", bento.to_toml());
    } else {
        write(dir, &bento, &themes, wallpaper.as_ref())?;
        eprintln!("Wrote {}:", dir.display());
        eprintln!("{}", summary(&bento, themes.len()));
    }
    if !left.is_empty() {
        eprintln!("Left out:");
        for item in &left {
            eprintln!("  {item}");
        }
    }
    if !options.print {
        eprintln!(
            "Add a description and screenshots in {}, then push the directory to a git\nrepository: `mochi bento add github.com/<you>/<repository>` installs it.",
            dir.join(FILE).display()
        );
    }
    Ok(())
}

fn write(
    dir: &Path,
    bento: &Bento,
    themes: &[(String, PathBuf)],
    wallpaper: Option<&(String, PathBuf)>,
) -> Result<(), String> {
    let io = |error: std::io::Error| format!("cannot write {}: {error}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(dir.join(FILE), bento.to_toml()).map_err(io)?;
    for (id, from) in themes {
        let to = dir.join("themes").join(id);
        std::fs::create_dir_all(&to).map_err(io)?;
        let manifest = mochi_core::themes::MANIFEST;
        std::fs::copy(from.join(manifest), to.join(manifest)).map_err(io)?;
    }
    if let Some((name, from)) = wallpaper {
        std::fs::copy(from, dir.join(name)).map_err(io)?;
    }
    Ok(())
}

/// One line per part, for after writing and before adding.
pub fn summary(bento: &Bento, themes: usize) -> String {
    let mut lines = Vec::new();
    let mut settings = Vec::new();
    leaves(&bento.config, &mut Vec::new(), &mut settings);
    let mut look = Vec::new();
    leaves(&bento.theme, &mut Vec::new(), &mut look);
    lines.push(format!(
        "  settings  {} options, {} of the theme",
        settings.len(),
        look.len()
    ));
    if themes > 0 {
        lines.push(format!("  themes    {themes} brought along"));
    }
    if !bento.plugins.is_empty() {
        let ids: Vec<&str> = bento.plugins.keys().map(String::as_str).collect();
        lines.push(format!("  plugins   {}", ids.join(", ")));
    }
    if !bento.widgets.is_empty() {
        let screens = bento
            .widgets
            .iter()
            .filter_map(|widget| screens::parse_role(&widget.output))
            .max()
            .map_or(1, |most| most + 1);
        lines.push(format!(
            "  widgets   {} on {screens} screen{}",
            bento.widgets.len(),
            if screens == 1 { "" } else { "s" }
        ));
    }
    if let Some(wallpaper) = &bento.bento.wallpaper {
        lines.push(format!("  wallpaper {wallpaper}"));
    }
    lines.join("\n")
}

/// The bento's id: the directory's name, made to fit.
fn id_from(dir: &Path) -> Result<String, String> {
    let name = std::fs::canonicalize(dir)
        .ok()
        .unwrap_or_else(|| dir.to_owned())
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let id: String = name
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char == '-' || char == '_' {
                char
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_start_matches(|char: char| !char.is_ascii_lowercase())
        .to_owned();
    mochi_plugins::manifest::check_id(&id)
        .map(|()| id)
        .map_err(|_| {
            format!("name the directory like the bento's id, such as `cozy`, not {name:?}")
        })
}

fn title(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Removes what looks personal from `table`, naming it in `left` under
/// `label`: keys that hold a place or a secret, paths in the home
/// directory, and values that look like tokens.
fn clean(table: &mut Table, label: &str, left: &mut Left) {
    let home = std::env::var("HOME").ok().filter(|home| home.len() > 1);
    let mut paths = Vec::new();
    leaves(table, &mut Vec::new(), &mut paths);
    for path in paths {
        let key = path.last().map(String::as_str).unwrap_or_default();
        let value = get(table, &path).cloned();
        let lower = key.to_lowercase();
        let why = if PLACE_KEYS.contains(&lower.as_str()) {
            Some("says where you are")
        } else if SECRET_KEYS.contains(&lower.as_str()) {
            Some("may be a secret")
        } else if value.as_ref().is_some_and(|value| {
            texts(value).any(|text| {
                home.as_ref()
                    .is_some_and(|home| text.starts_with(&format!("{home}/")) || text == home)
            })
        }) {
            Some("a path in your home directory")
        } else if value
            .as_ref()
            .is_some_and(|value| texts(value).any(looks_secret))
        {
            Some("looks like a token")
        } else {
            None
        };
        if let Some(why) = why {
            remove(table, &path);
            left.push(format!("{label}.{}: {why}", path.join(".")));
        }
    }
}

/// The strings in a value: itself, or a list's.
fn texts(value: &Value) -> Box<dyn Iterator<Item = &str> + '_> {
    match value {
        Value::String(text) => Box::new(std::iter::once(text.as_str())),
        Value::Array(items) => Box::new(items.iter().flat_map(texts)),
        _ => Box::new(std::iter::empty()),
    }
}

/// A long run of letters and digits mixed, with nothing a word, a path, a
/// command or a color has.
fn looks_secret(text: &str) -> bool {
    text.len() >= 24
        && text
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || "-_".contains(char))
        && text.chars().any(|char| char.is_ascii_digit())
        && text.chars().any(|char| char.is_ascii_alphabetic())
}

/// Every value that isn't a table, by path. Lists count as one value.
fn leaves(table: &Table, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    for (key, value) in table {
        prefix.push(key.clone());
        match value {
            Value::Table(inner) => leaves(inner, prefix, out),
            _ => out.push(prefix.clone()),
        }
        prefix.pop();
    }
}

fn get<'a>(table: &'a Table, path: &[String]) -> Option<&'a Value> {
    let parts: Vec<&str> = path.iter().map(String::as_str).collect();
    mochi_core::changes::get(table, &parts)
}

fn remove(table: &mut Table, path: &[String]) {
    let parts: Vec<&str> = path.iter().map(String::as_str).collect();
    mochi_core::changes::remove(table, &parts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_values_are_left_out() {
        let mut table: Table = mochi_core::toml::from_str(&format!(
            r#"
modules = ["idle", "nightlight"]
[module.nightlight]
latitude = 48.85
temperature = 4000
[module.weather]
api_key = "x"
token_like = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4"
[module.capture]
directory = "{home}/Videos"
portable = "~/Videos"
editor = ["satty", "--filename"]
"#,
            home = std::env::var("HOME").unwrap()
        ))
        .unwrap();
        let mut left = Left::new();
        clean(&mut table, "config", &mut left);
        let kept = mochi_core::toml::to_string(&table).unwrap();
        assert!(kept.contains("temperature = 4000"), "{kept}");
        assert!(kept.contains("portable"), "{kept}");
        assert!(kept.contains("satty"), "{kept}");
        for gone in ["latitude", "api_key", "token_like", "directory"] {
            assert!(!kept.contains(gone), "{gone} stayed: {kept}");
        }
        assert_eq!(left.len(), 4, "{left:?}");
        assert!(
            left.iter()
                .any(|item| item.starts_with("config.module.nightlight.latitude"))
        );
    }

    #[test]
    fn ids_and_titles_come_from_the_directory() {
        assert_eq!(
            id_from(Path::new("/tmp/My Cozy Desk")).unwrap(),
            "my-cozy-desk"
        );
        assert_eq!(title("my-cozy_desk"), "My Cozy Desk");
        assert!(id_from(Path::new("/tmp/42")).is_err());
    }
}

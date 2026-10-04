//! The commented `config.toml` and `theme.toml` that `mochid` writes when a
//! user has none, and that the documentation shows.
//!
//! Every section comes from an example file next to the code that reads it:
//! the theme and bubbles here, and each module's own `settings.toml`. A line
//! like `# timeout_ms = 1500` shows a default; tests uncomment those lines
//! and check they parse to the real defaults, so the examples can't drift.

use std::fmt::Debug;

use serde::de::DeserializeOwned;

/// `theme.toml`, every token at its default.
pub const THEME: &str = include_str!("../defaults/theme.toml");

/// The `[island]` section of `config.toml`.
pub const ISLAND: &str = include_str!("../defaults/island.toml");

/// The `[bubbles]` section of `config.toml`.
pub const BUBBLES: &str = include_str!("../defaults/bubbles.toml");

/// The TOML an example stands for: each `# key = value` line uncommented.
/// Other comments, like a commented `# [bubbles.media]`, stay comments.
pub fn uncommented(example: &str) -> String {
    example
        .lines()
        .map(|line| match line.strip_prefix("# ") {
            Some(rest) if is_default(rest) => rest,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `key = value`, where the key is a bare TOML key.
fn is_default(line: &str) -> bool {
    line.split_once(" = ").is_some_and(|(key, _)| {
        !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    })
}

/// `config.toml` with `enabled` turned on and every module's example after
/// it. `modules` pairs each available module with its example.
pub fn config(enabled: &[&str], modules: &[(&str, &str)]) -> String {
    let available: Vec<&str> = modules.iter().map(|(id, _)| *id).collect();
    let mut text = String::from(
        "# Mochi's settings. mochid wrote this file because there was none, and\n\
         # it never overwrites it.\n\
         #\n\
         # A line like \"# timeout_ms = 1500\" shows a default: remove the \"# \" to\n\
         # change it. `mochi config check` checks the file and `mochi reload`\n\
         # applies it without a restart.\n\
         \n",
    );
    text.push_str(&wrap(
        &format!(
            "The modules to run, in this order. Available: {}.",
            available.join(", ")
        ),
        76,
    ));
    text.push_str("modules = [\n");
    for id in enabled {
        text.push_str(&format!("    \"{id}\",\n"));
    }
    text.push_str("]\n\n");
    text.push_str(ISLAND.trim_end());
    text.push_str("\n\n");
    text.push_str(BUBBLES.trim_end());
    for (_, example) in modules {
        if !example.is_empty() {
            text.push_str("\n\n");
            text.push_str(example.trim_end());
        }
    }
    text.push('\n');
    text
}

/// `text` as `# ` comment lines at most `width` wide.
fn wrap(text: &str, width: usize) -> String {
    let mut lines = vec![String::from("#")];
    for word in text.split_whitespace() {
        let line = lines.last_mut().expect("starts with one line");
        if line.len() + 1 + word.len() > width && line.len() > 1 {
            lines.push(format!("# {word}"));
        } else {
            line.push(' ');
            line.push_str(word);
        }
    }
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// Panics unless `example` is the `[module.<module>]` section and its
/// defaults parse to `T::default()`. For a test in each module.
pub fn check_module<T>(module: &str, example: &str)
where
    T: DeserializeOwned + Default + PartialEq + Debug,
{
    let header = format!("[module.{module}]");
    assert!(
        example.lines().any(|line| line == header),
        "the example for {module} has no {header} line"
    );
    let table: toml::Table = toml::from_str(&uncommented(example))
        .unwrap_or_else(|error| panic!("the example for {module} doesn't parse: {error}"));
    let section = table
        .get("module")
        .and_then(|modules| modules.get(module))
        .cloned()
        .unwrap_or_else(|| toml::Value::Table(toml::Table::new()));
    let parsed: T = section
        .try_into()
        .unwrap_or_else(|error| panic!("the example for {module} has a wrong setting: {error}"));
    assert_eq!(
        parsed,
        T::default(),
        "the example for {module} doesn't show its defaults"
    );
}

#[cfg(test)]
mod tests {
    use mochi_protocol::Theme;

    use super::*;
    use crate::config::{BubblesConfig, Config, IslandConfig};

    #[test]
    fn uncomments_defaults_only() {
        let example = "# Prose stays.\n[module.osd]\n# timeout_ms = 1500\n# [bubbles.media]\n#   like this = 1";
        assert_eq!(
            uncommented(example),
            "# Prose stays.\n[module.osd]\ntimeout_ms = 1500\n# [bubbles.media]\n#   like this = 1"
        );
    }

    #[test]
    fn the_theme_example_shows_the_defaults() {
        let theme: Theme = toml::from_str(&uncommented(THEME)).unwrap();
        assert_eq!(theme, Theme::default());
    }

    #[test]
    fn the_island_example_shows_the_defaults() {
        let config: Config = toml::from_str(&uncommented(ISLAND)).unwrap();
        assert_eq!(config.island, IslandConfig::default());
    }

    #[test]
    fn the_bubbles_example_shows_the_defaults() {
        let config: Config = toml::from_str(&uncommented(BUBBLES)).unwrap();
        assert_eq!(config.bubbles, BubblesConfig::default());
    }

    #[test]
    fn the_generated_config_parses_with_its_modules() {
        let osd = "[module.osd]\n# timeout_ms = 1500";
        let text = config(&["idle", "osd"], &[("idle", ""), ("osd", osd)]);
        let parsed = Config::parse(&text, std::path::Path::new("config.toml")).unwrap();
        assert_eq!(parsed.modules, ["idle", "osd"]);
        // Defaults stay commented, so the code's defaults apply.
        assert!(parsed.settings("osd").is_empty());
        assert!(text.contains("Available: idle, osd."));
        assert!(text.lines().all(|line| line.len() <= 80), "{text}");
    }
}

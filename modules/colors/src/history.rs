//! The colors picked, newest first, kept in
//! `$XDG_STATE_HOME/mochi/colors.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::color::Color;

/// `$XDG_STATE_HOME/mochi/colors.json`, falling back to `~/.local/state`.
pub fn state_file() -> Option<PathBuf> {
    Some(mochi_core::config::state_dir()?.join("colors.json"))
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct File {
    /// Each color as `#rrggbb`, or `#rrggbbaa`.
    colors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    colors: Vec<Color>,
    limit: usize,
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            colors: Vec::new(),
            limit,
        }
    }

    /// The saved colors, or none when the file is missing or broken.
    /// Colors past `limit` go.
    pub fn load(path: &Path, limit: usize) -> Self {
        let file: File = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut history = Self::new(limit);
        // Oldest first, so the newest ends up on top.
        for color in file
            .colors
            .iter()
            .rev()
            .filter_map(|text| Color::parse(text))
        {
            history.add(color);
        }
        history
    }

    /// Writes a temporary file next to it, then renames it over, so a crash
    /// never leaves half a file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = File {
            colors: self.colors.iter().map(|color| color.id()).collect(),
        };
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&file)?)?;
        std::fs::rename(temporary, path)
    }

    pub fn colors(&self) -> &[Color] {
        &self.colors
    }

    /// Puts a color on top, moving it there when it's already in.
    pub fn add(&mut self, color: Color) {
        self.colors.retain(|kept| *kept != color);
        self.colors.insert(0, color);
        self.colors.truncate(self.limit);
    }

    /// Whether it was there.
    pub fn remove(&mut self, color: Color) -> bool {
        let before = self.colors.len();
        self.colors.retain(|kept| *kept != color);
        self.colors.len() != before
    }

    pub fn clear(&mut self) {
        self.colors.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(text: &str) -> Color {
        Color::parse(text).unwrap()
    }

    #[test]
    fn newest_first_once_each_and_capped() {
        let mut history = History::new(3);
        for text in ["#111111", "#222222", "#111111", "#333333", "#444444"] {
            history.add(color(text));
        }
        let ids: Vec<String> = history.colors().iter().map(|color| color.id()).collect();
        assert_eq!(ids, ["#444444", "#333333", "#111111"]);
        assert!(history.remove(color("#333333")));
        assert!(!history.remove(color("#333333")));
        assert_eq!(history.colors().len(), 2);
        history.clear();
        assert!(history.colors().is_empty());
        // No room at all keeps nothing.
        let mut none = History::new(0);
        none.add(color("#111111"));
        assert!(none.colors().is_empty());
    }

    #[test]
    fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("mochi-colors-{}", std::process::id()));
        let path = dir.join("colors.json");
        let mut history = History::new(10);
        history.add(color("#1e1e2e"));
        history.add(color("#1e1e2e80"));
        history.save(&path).unwrap();
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(History::load(&path, 10), history);
        // A smaller limit keeps the newest.
        assert_eq!(History::load(&path, 1).colors(), [color("#1e1e2e80")]);

        std::fs::write(&path, r##"{"colors": ["#abc", "nonsense", "#ABC"]}"##).unwrap();
        assert_eq!(History::load(&path, 10).colors(), [color("#aabbcc")]);
        std::fs::write(&path, "broken").unwrap();
        assert_eq!(History::load(&path, 10), History::new(10));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

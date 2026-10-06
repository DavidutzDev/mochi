//! The emoji picked last, most recent first, kept in
//! `$XDG_STATE_HOME/mochi/emoji.json` as a JSON list.

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Recents {
    list: Vec<String>,
    cap: usize,
}

impl Recents {
    pub fn new(cap: usize) -> Self {
        Self {
            list: Vec::new(),
            cap,
        }
    }

    pub fn list(&self) -> &[String] {
        &self.list
    }

    pub fn contains(&self, id: &str) -> bool {
        self.list.iter().any(|recent| recent == id)
    }

    /// Puts an emoji first, dropping the oldest past the cap.
    pub fn push(&mut self, id: &str) {
        self.list.retain(|recent| recent != id);
        self.list.insert(0, id.to_owned());
        self.list.truncate(self.cap);
    }

    /// Reads the file. A missing or broken one leaves the list empty.
    pub fn load(&mut self, file: &Path) {
        self.list = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
            .unwrap_or_default();
        self.list.truncate(self.cap);
    }

    /// Writes the file through a temporary one, so it's never half written.
    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temporary = file.with_extension("json.tmp");
        let text = serde_json::to_string(&self.list).map_err(std::io::Error::other)?;
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, file)
    }
}

/// `$XDG_STATE_HOME/mochi/emoji.json`, falling back to `~/.local/state`.
pub fn file() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/state")))?;
    Some(state.join("mochi").join("emoji.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn most_recent_first_and_capped() {
        let mut recents = Recents::new(3);
        for id in ["a", "b", "c", "d", "b"] {
            recents.push(id);
        }
        assert_eq!(recents.list(), ["b", "d", "c"]);
        assert!(recents.contains("c"));
        assert!(!recents.contains("a"));
    }

    #[test]
    fn a_cap_of_zero_keeps_none() {
        let mut recents = Recents::new(0);
        recents.push("a");
        assert!(recents.list().is_empty());
    }

    #[test]
    fn they_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("mochi-emoji-recents-{}", std::process::id()));
        let file = dir.join("mochi/emoji.json");
        let mut recents = Recents::new(8);
        recents.push("🐱");
        recents.push("😀");
        recents.save(&file).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), r#"["😀","🐱"]"#);
        assert!(!file.with_extension("json.tmp").exists());

        let mut again = Recents::new(8);
        again.load(&file);
        assert_eq!(again.list(), ["😀", "🐱"]);
        // A smaller cap than before keeps the newest.
        let mut fewer = Recents::new(1);
        fewer.load(&file);
        assert_eq!(fewer.list(), ["😀"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_broken_file_is_empty() {
        let dir = std::env::temp_dir().join(format!("mochi-emoji-broken-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("emoji.json");
        std::fs::write(&file, "not json").unwrap();
        let mut recents = Recents::new(8);
        recents.load(&file);
        assert!(recents.list().is_empty());
        recents.load(&dir.join("missing.json"));
        assert!(recents.list().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

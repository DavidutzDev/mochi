//! The emoji picked last, most recent first, and the skin tones, kept in
//! `$XDG_STATE_HOME/mochi/emoji.json`:
//!
//! ```json
//! {"recent": ["👍", "🐱"], "tone": "medium", "tones": {"👋": "dark"}}
//! ```
//!
//! The recents are the emoji without a tone. Before tones, the file was the
//! list alone, which still reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tones::{Tone, Tones};

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
}

/// The file's contents.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    recent: Vec<String>,
    tone: Tone,
    tones: BTreeMap<String, Tone>,
}

/// Reads the file. A missing or broken one leaves both empty.
pub fn load(file: &Path, recents: &mut Recents, tones: &mut Tones) {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let saved = serde_json::from_str::<Saved>(&text)
        .or_else(|_| {
            serde_json::from_str::<Vec<String>>(&text).map(|recent| Saved {
                recent,
                ..Saved::default()
            })
        })
        .unwrap_or_default();
    recents.list = saved.recent;
    recents.list.truncate(recents.cap);
    *tones = Tones {
        default: saved.tone,
        chosen: saved.tones,
    };
}

/// Writes the file through a temporary one, so it's never half written.
pub fn save(file: &Path, recents: &Recents, tones: &Tones) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temporary = file.with_extension("json.tmp");
    let saved = Saved {
        recent: recents.list.clone(),
        tone: tones.default,
        tones: tones.chosen.clone(),
    };
    let text = serde_json::to_string(&saved).map_err(std::io::Error::other)?;
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, file)
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
        let mut tones = Tones {
            default: Tone::MediumLight,
            ..Tones::default()
        };
        tones.chosen.insert("👋".into(), Tone::Dark);
        save(&file, &recents, &tones).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            r#"{"recent":["😀","🐱"],"tone":"medium-light","tones":{"👋":"dark"}}"#
        );
        assert!(!file.with_extension("json.tmp").exists());

        let mut again = Recents::new(8);
        let mut tones_again = Tones::default();
        load(&file, &mut again, &mut tones_again);
        assert_eq!(again.list(), ["😀", "🐱"]);
        assert_eq!(tones_again, tones);
        // A smaller cap than before keeps the newest.
        let mut fewer = Recents::new(1);
        load(&file, &mut fewer, &mut tones_again);
        assert_eq!(fewer.list(), ["😀"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_old_list_still_reads() {
        let dir = std::env::temp_dir().join(format!("mochi-emoji-old-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("emoji.json");
        std::fs::write(&file, r#"["😀","🐱"]"#).unwrap();
        let mut recents = Recents::new(8);
        let mut tones = Tones::default();
        load(&file, &mut recents, &mut tones);
        assert_eq!(recents.list(), ["😀", "🐱"]);
        assert_eq!(tones, Tones::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_broken_file_is_empty() {
        let dir = std::env::temp_dir().join(format!("mochi-emoji-broken-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("emoji.json");
        std::fs::write(&file, "not json").unwrap();
        let mut recents = Recents::new(8);
        let mut tones = Tones::default();
        load(&file, &mut recents, &mut tones);
        assert!(recents.list().is_empty());
        load(&dir.join("missing.json"), &mut recents, &mut tones);
        assert!(recents.list().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

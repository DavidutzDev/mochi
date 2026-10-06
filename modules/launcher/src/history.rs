//! What the user launched and when, so often used apps come first.
//!
//! Each launch adds 1 to an app's score, and scores halve every 30 days, so
//! an app used daily last year falls behind one used weekly now. Saved as
//! JSON under `$XDG_STATE_HOME/mochi/`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const HALF_LIFE_SECS: f64 = 30.0 * 24.0 * 3600.0;

#[derive(Debug, Default)]
pub struct History {
    path: Option<PathBuf>,
    uses: HashMap<String, Use>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Use {
    score: f64,
    /// Seconds since the epoch.
    last: u64,
}

impl History {
    /// Reads the file, or starts empty when there is none or it's broken.
    pub fn load(path: PathBuf) -> Self {
        let uses = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            path: Some(path),
            uses,
        }
    }

    /// `$XDG_STATE_HOME/mochi/launcher.json`, falling back to
    /// `~/.local/state`.
    pub fn default_path() -> Option<PathBuf> {
        let state = std::env::var_os("XDG_STATE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/state"))
            })?;
        Some(state.join("mochi").join("launcher.json"))
    }

    /// The score at `now`, 0 for something never launched.
    pub fn score(&self, id: &str, now: u64) -> f64 {
        self.uses.get(id).map_or(0.0, |entry| decayed(*entry, now))
    }

    /// The ids starting with `prefix`, without it, highest score first.
    pub fn starting_with(&self, prefix: &str, now: u64) -> Vec<String> {
        let mut found: Vec<(&str, f64)> = self
            .uses
            .iter()
            .filter_map(|(id, entry)| Some((id.strip_prefix(prefix)?, decayed(*entry, now))))
            .collect();
        found.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        found.into_iter().map(|(id, _)| id.to_owned()).collect()
    }

    /// Counts a launch and saves.
    pub fn record(&mut self, id: &str, now: u64) {
        let score = self.score(id, now) + 1.0;
        self.uses.insert(id.to_owned(), Use { score, last: now });
        if let Err(error) = self.save() {
            tracing::warn!(%error, "could not save the launch history");
        }
    }

    fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Through a temporary file, so a crash never leaves half a file.
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec(&self.uses)?)?;
        std::fs::rename(temporary, path)
    }
}

fn decayed(entry: Use, now: u64) -> f64 {
    let elapsed = now.saturating_sub(entry.last) as f64;
    entry.score * 0.5_f64.powf(elapsed / HALF_LIFE_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 24 * 3600;

    #[test]
    fn scores_add_up_and_fade() {
        let mut history = History::default();
        history.record("a", 0);
        history.record("a", 0);
        assert_eq!(history.score("a", 0), 2.0);
        assert!((history.score("a", 30 * DAY) - 1.0).abs() < 1e-9);
        assert_eq!(history.score("b", 0), 0.0);

        // Recent use beats old heavy use.
        history.record("b", 90 * DAY);
        assert!(history.score("b", 90 * DAY) > history.score("a", 90 * DAY));
    }

    #[test]
    fn survives_a_restart_and_a_broken_file() {
        let dir = std::env::temp_dir().join(format!("mochi-history-{}", std::process::id()));
        let path = dir.join("launcher.json");
        let mut history = History::load(path.clone());
        history.record("firefox.desktop", 100);
        assert_eq!(
            History::load(path.clone()).score("firefox.desktop", 100),
            1.0
        );

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(History::load(path).score("firefox.desktop", 100), 0.0);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

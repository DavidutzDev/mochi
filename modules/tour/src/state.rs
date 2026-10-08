//! `$XDG_STATE_HOME/mochi/tour.json`: which release's tour the user saw or
//! turned down, and where a stopped tour was.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::steps::Version;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    /// The last release whose tour was taken, stopped or turned down. Empty
    /// before the first.
    pub seen: String,
    /// "Never" was picked: no more offers.
    pub never: bool,
    /// The step a stopped tour was at, by id, and whether it was a tour of
    /// what's new.
    pub resume: Option<String>,
    pub resume_news: bool,
}

impl Saved {
    pub fn path() -> Option<PathBuf> {
        Some(mochi_core::config::state_dir()?.join("tour.json"))
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::write(path, serde_json::to_vec_pretty(self).unwrap_or_default())
            });
        if let Err(error) = written {
            tracing::warn!(%error, "can't save the tour's state");
        }
    }

    pub fn seen(&self) -> Option<Version> {
        Version::parse(&self.seen)
    }
}

/// What to offer at startup: the whole tour the first time, a tour of
/// what's new after an update, or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    Whole,
    News(Version),
}

pub fn offer(saved: &Saved, install: bool, updates: bool, current: Version) -> Option<Offer> {
    if saved.never {
        return None;
    }
    match saved.seen() {
        None => install.then_some(Offer::Whole),
        Some(seen) if seen < current && updates => Some(Offer::News(seen)),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_the_whole_tour_once_then_updates() {
        let now = Version(0, 0, 8);
        let mut saved = Saved::default();
        assert_eq!(offer(&saved, true, true, now), Some(Offer::Whole));
        assert_eq!(offer(&saved, false, true, now), None);
        saved.seen = "0.0.8".into();
        assert_eq!(offer(&saved, true, true, now), None);
        assert_eq!(
            offer(&saved, true, true, Version(0, 0, 9)),
            Some(Offer::News(now))
        );
        assert_eq!(offer(&saved, true, false, Version(0, 0, 9)), None);
        saved.never = true;
        assert_eq!(offer(&saved, true, true, Version(0, 0, 9)), None);
    }

    #[test]
    fn the_state_round_trips() {
        let path = std::env::temp_dir().join(format!("mochi-tour-{}.json", std::process::id()));
        let saved = Saved {
            seen: "0.0.8".into(),
            never: false,
            resume: Some("media/player".into()),
            resume_news: false,
        };
        saved.save(&path);
        assert_eq!(Saved::load(&path), saved);
        std::fs::remove_file(path).unwrap();
    }
}

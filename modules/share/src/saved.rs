//! What the share module keeps across a restart of mochid, in its session
//! directory: what Hyprland captures, which it only reports when a capture
//! starts or stops, and the switchable share, whose monitor the app keeps
//! capturing while Mochi restarts. Without them, a restart forgot the share
//! was running and removed the monitor under the app, which froze.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mochi_core::quality::Resolution;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::switch::{Session, Source};

const FILE: &str = "share.json";

/// How old a save may be and still say what runs now. Every change and
/// every stop saves, so only a crash during a long share leaves an old one,
/// and then the share may have ended since.
const FRESH: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    /// Seconds since the epoch.
    pub saved_at: u64,
    /// What Hyprland captures, by monitor or window name.
    pub captured: Vec<String>,
    pub session: Option<SavedSession>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedSession {
    pub source: Value,
    pub framerate: u32,
    pub resolution: String,
    pub size: (u32, u32),
}

impl SavedSession {
    pub fn new(session: &Session) -> Self {
        Self {
            source: session.source.to_json(),
            framerate: session.framerate,
            resolution: session.resolution.as_str().to_owned(),
            size: session.size,
        }
    }

    /// The share again, as running: it was shared before the restart.
    pub fn resume(&self, now: std::time::Instant) -> Option<Session> {
        let mut session = Session::resumed(Source::from_json(&self.source)?, now);
        session.framerate = self.framerate;
        session.resolution = Resolution::parse(&self.resolution).unwrap_or_default();
        session.size = self.size;
        Some(session)
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

impl Saved {
    pub fn new(captured: Vec<String>, session: Option<&Session>) -> Self {
        Self {
            saved_at: now(),
            captured,
            session: session.map(SavedSession::new),
        }
    }

    /// The last save in `dir`, if it's recent enough to trust.
    pub fn load(dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(dir.join(FILE)).ok()?;
        let saved: Self = serde_json::from_str(&text)
            .inspect_err(|error| tracing::warn!(%error, "ignoring the saved share"))
            .ok()?;
        (now().saturating_sub(saved.saved_at) <= FRESH.as_secs()).then_some(saved)
    }

    pub fn save(&self, dir: &Path) {
        let written = std::fs::create_dir_all(dir).and_then(|()| {
            let temporary = dir.join(format!("{FILE}.new"));
            std::fs::write(&temporary, serde_json::to_vec(self).unwrap_or_default())?;
            std::fs::rename(temporary, dir.join(FILE))
        });
        if let Err(error) = written {
            tracing::warn!(%error, "can't save the share's state");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn a_share_survives_a_save_and_a_load() {
        let dir = std::env::temp_dir().join(format!("mochi-share-{}", std::process::id()));
        let mut session = Session::new(
            Source::Area {
                output: "DP-3".into(),
                x: 10,
                y: 20,
                width: 640,
                height: 360,
            },
            Instant::now(),
        );
        session.framerate = 30;
        session.resolution = Resolution::P720;
        session.size = (1280, 720);
        Saved::new(vec!["MOCHI-SHARE".into()], Some(&session)).save(&dir);

        let saved = Saved::load(&dir).unwrap();
        assert_eq!(saved.captured, ["MOCHI-SHARE"]);
        let resumed = saved.session.unwrap().resume(Instant::now()).unwrap();
        assert_eq!(resumed.source, session.source);
        assert_eq!(resumed.framerate, 30);
        assert_eq!(resumed.resolution, Resolution::P720);
        assert_eq!(resumed.size, (1280, 720));

        // An old save says nothing about now.
        let mut old = Saved::new(Vec::new(), None);
        old.saved_at -= FRESH.as_secs() + 1;
        old.save(&dir);
        assert_eq!(Saved::load(&dir), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

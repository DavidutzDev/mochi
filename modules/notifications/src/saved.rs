//! The history kept across restarts, as JSON under `$XDG_STATE_HOME/mochi/`,
//! readable only by the user: notifications can hold things like login
//! codes.
//!
//! Actions aren't kept: the apps that sent them are another connection by
//! then, and wouldn't know the notification anymore. Images from raw pixels
//! are PNGs in the `notifications` directory next to the file, so they last
//! too.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::note::{Image, Note, Urgency};

/// `$XDG_STATE_HOME/mochi`, falling back to `~/.local/state/mochi`.
pub fn dir() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/state")))?;
    Some(state.join("mochi"))
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Saved {
    id: u32,
    app: String,
    icon: String,
    summary: String,
    body: String,
    urgency: String,
    /// A path or an icon name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    received_ms: u64,
}

/// The notifications in the file, oldest first. None when there is no file
/// or it's broken.
pub fn load(path: &Path) -> Vec<Note> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let saved: Vec<Saved> = match serde_json::from_str(&text) {
        Ok(saved) => saved,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "the saved notifications don't read; starting without them");
            return Vec::new();
        }
    };
    saved
        .into_iter()
        .map(|saved| Note {
            id: saved.id,
            app: saved.app,
            icon: saved.icon,
            summary: saved.summary,
            body: saved.body,
            actions: Vec::new(),
            urgency: match saved.urgency.as_str() {
                "low" => Urgency::Low,
                "critical" => Urgency::Critical,
                _ => Urgency::Normal,
            },
            timeout: None,
            image: saved.image.map(Image::Path),
            resident: false,
            transient: false,
            sound: None,
            received: SystemTime::UNIX_EPOCH + Duration::from_millis(saved.received_ms),
        })
        .collect()
}

/// Writes `notes`, oldest first, through a temporary file so a crash never
/// leaves half of one.
pub fn save<'a>(path: &Path, notes: impl Iterator<Item = &'a Note>) -> std::io::Result<()> {
    let saved: Vec<Saved> = notes
        .map(|note| Saved {
            id: note.id,
            app: note.app.clone(),
            icon: note.icon.clone(),
            summary: note.summary.clone(),
            body: note.body.clone(),
            urgency: note.urgency.as_str().to_owned(),
            image: match &note.image {
                Some(Image::Path(path)) => Some(path.clone()),
                _ => None,
            },
            received_ms: note
                .received
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |since| {
                    u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
                }),
        })
        .collect();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(&saved)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, path)
}

/// Deletes the PNGs in `dir` that no kept notification shows anymore.
pub fn tidy(dir: &Path, notes: &[Note]) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let shown = notes.iter().any(
            |note| matches!(&note.image, Some(Image::Path(image)) if Path::new(image) == path),
        );
        if !shown && path.extension().is_some_and(|extension| extension == "png") {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::note::Action;

    #[test]
    fn keeps_the_history_but_not_the_actions() {
        let dir = std::env::temp_dir().join(format!("mochi-notes-{}", std::process::id()));
        let path = dir.join("notifications.json");
        let note = Note {
            id: 7,
            app: "Signal".into(),
            icon: "signal-desktop".into(),
            summary: "Ana".into(),
            body: "Lunch?".into(),
            actions: vec![Action {
                key: "default".into(),
                label: "Open".into(),
            }],
            urgency: Urgency::Critical,
            timeout: Some(Duration::from_secs(3)),
            image: Some(Image::Path("/tmp/ana.png".into())),
            resident: true,
            transient: false,
            sound: None,
            received: SystemTime::UNIX_EPOCH + Duration::from_millis(1_700_000_000_123),
        };
        save(&path, [&note].into_iter()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);

        let loaded = load(&path);
        assert_eq!(loaded.len(), 1);
        let back = &loaded[0];
        assert_eq!((back.id, back.summary.as_str()), (7, "Ana"));
        assert_eq!(back.urgency, Urgency::Critical);
        assert_eq!(back.image, note.image);
        assert_eq!(back.received, note.received);
        assert!(back.actions.is_empty() && !back.resident);

        std::fs::write(&path, "not json").unwrap();
        assert!(load(&path).is_empty());
        assert!(load(&dir.join("missing.json")).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

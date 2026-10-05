//! The captures history for the hub page: the newest screenshots and
//! recordings in their folders, whatever made them.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{Value, json};

/// How many captures the page lists.
pub const SHOWN: usize = 40;

const PICTURES: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
const VIDEOS: [&str; 4] = ["mp4", "mkv", "webm", "mov"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    /// A picture rather than a video.
    pub screenshot: bool,
    pub modified: SystemTime,
    pub bytes: u64,
}

impl Entry {
    pub fn to_json(&self) -> Value {
        let seconds = self
            .modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        json!({
            "path": self.path.display().to_string(),
            "name": self.path.file_stem().map(|name| name.to_string_lossy().into_owned()),
            "kind": if self.screenshot { "screenshot" } else { "recording" },
            "time": seconds,
            "bytes": self.bytes,
        })
    }
}

/// The newest captures in `folders`, newest first, at most `limit`.
/// Folders that don't exist yet add nothing.
pub fn scan(folders: &[&Path], limit: usize) -> Vec<Entry> {
    let mut entries: Vec<Entry> = folders
        .iter()
        .filter_map(|folder| std::fs::read_dir(folder).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|file| {
            let path = file.path();
            let extension = path.extension()?.to_str()?.to_ascii_lowercase();
            let screenshot = if PICTURES.contains(&extension.as_str()) {
                true
            } else if VIDEOS.contains(&extension.as_str()) {
                false
            } else {
                return None;
            };
            let metadata = file.metadata().ok().filter(std::fs::Metadata::is_file)?;
            Some(Entry {
                path,
                screenshot,
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                bytes: metadata.len(),
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.path.cmp(&b.path))
    });
    entries.dedup_by(|a, b| a.path == b.path);
    entries.truncate(limit);
    entries
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::time::Duration;

    use super::*;

    fn touch(path: &Path, seconds: u64) {
        let file = File::create(path).unwrap();
        file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
            .unwrap();
    }

    #[test]
    fn lists_captures_newest_first() {
        let dir = std::env::temp_dir().join(format!("mochi-history-{}", std::process::id()));
        let (pictures, videos) = (dir.join("pictures"), dir.join("videos"));
        std::fs::create_dir_all(&pictures).unwrap();
        std::fs::create_dir_all(&videos).unwrap();
        touch(&pictures.join("old.png"), 100);
        touch(&pictures.join("new.PNG"), 300);
        touch(&pictures.join("notes.txt"), 400);
        touch(&videos.join("clip.mp4"), 200);
        std::fs::create_dir_all(videos.join("folder.mp4")).unwrap();

        let entries = scan(&[&pictures, &videos, &dir.join("missing")], 10);
        let names: Vec<_> = entries
            .iter()
            .map(|entry| entry.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["new.PNG", "clip.mp4", "old.png"]);
        assert!(!entries[1].screenshot);
        assert_eq!(entries[1].to_json()["kind"], "recording");
        assert_eq!(entries[0].to_json()["name"], "new");
        assert_eq!(entries[0].to_json()["time"], 300);

        assert_eq!(scan(&[&pictures, &videos], 1).len(), 1);
        // The same folder twice lists each file once.
        assert_eq!(scan(&[&pictures, &pictures], 10).len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

//! Thumbnails of recordings: one frame of each video, made once with
//! ffmpeg and kept in `$XDG_CACHE_HOME/mochi/thumbnails`, named after the
//! video's path and when it last changed, so an edited video gets a new
//! one.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tokio::process::Command;

/// How wide thumbnails are, in pixels; the page and the card show them
/// smaller, sharp on a scaled screen.
const WIDTH: u32 = 320;

/// The folder thumbnails go in.
pub fn folder() -> Option<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache")))?;
    Some(cache.join("mochi/thumbnails"))
}

/// Where the thumbnail of `video`, as it was at `modified`, goes in `folder`.
pub fn path(folder: &Path, video: &Path, modified: SystemTime) -> PathBuf {
    let seconds = modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let mut hasher = blake3::Hasher::new();
    hasher.update(video.as_os_str().as_encoded_bytes());
    hasher.update(&seconds.to_le_bytes());
    folder.join(format!("{}.jpg", &hasher.finalize().to_hex()[..32]))
}

/// Makes the thumbnail of `video` at `out`: a frame ffmpeg finds typical
/// among the first ones, so a fade from black doesn't make it black.
/// Written beside and renamed, so a half-written file never shows.
pub async fn make(video: &Path, out: &Path) -> Result<(), String> {
    if let Some(parent) = out.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("can't make {}: {error}", parent.display()))?;
    }
    let partial = out.with_extension("part.jpg");
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-loglevel", "error", "-y", "-i"])
        .arg(video)
        .args([
            "-vf",
            &format!("thumbnail=60,scale={WIDTH}:-2"),
            "-frames:v",
            "1",
            "-q:v",
            "4",
        ])
        .arg(&partial)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("can't run ffmpeg: {error}"))?;
    if !output.status.success() || !partial.is_file() {
        let _ = tokio::fs::remove_file(&partial).await;
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg made no thumbnail: {}", said.trim()));
    }
    tokio::fs::rename(&partial, out)
        .await
        .map_err(|error| format!("can't keep the thumbnail: {error}"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_changed_video_gets_a_new_thumbnail() {
        let folder = Path::new("/cache");
        let video = Path::new("/videos/clip.mp4");
        let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let first = path(folder, video, then);
        assert_eq!(first, path(folder, video, then));
        assert_eq!(first.parent(), Some(folder));
        assert_eq!(first.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_ne!(first, path(folder, video, then + Duration::from_secs(1)));
        assert_ne!(first, path(folder, Path::new("/videos/other.mp4"), then));
    }
}

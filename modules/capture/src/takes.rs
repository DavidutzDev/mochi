//! A recording that changed screens: one file per screen, kept in a scratch
//! folder beside the recording, then joined into it once it stops.
//!
//! gpu-screen-recorder records one source from start to end, so changing
//! the screen stops it, which finishes the file, and starts it again on the
//! other one. The parts join with ffmpeg: copied as they are when they
//! share a size, which is instant, or else encoded again, each scaled into
//! the first one's size on black.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

/// The parts of a recording that changed screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Takes {
    /// Where the joined recording goes.
    pub file: PathBuf,
    /// The scratch folder the parts are in.
    pub dir: PathBuf,
    /// The parts finished so far, in order.
    pub parts: Vec<PathBuf>,
}

impl Takes {
    /// Starts keeping parts for `file`, moving what it recorded so far into
    /// the scratch folder as the first part.
    pub fn begin(file: &Path) -> std::io::Result<Self> {
        let stem = file
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parent = file.parent().unwrap_or_else(|| Path::new("."));
        let dir = parent.join(format!(".mochi-parts-{stem}"));
        std::fs::create_dir_all(&dir)?;
        let first = dir.join(format!("0.{}", extension(file)));
        std::fs::rename(file, &first)?;
        Ok(Self {
            file: file.to_owned(),
            dir,
            parts: vec![first],
        })
    }

    /// Where the next part goes.
    pub fn next(&self) -> PathBuf {
        self.dir
            .join(format!("{}.{}", self.parts.len(), extension(&self.file)))
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mp4".to_owned())
}

/// Joins the parts into the recording's file, and removes the scratch
/// folder. Without ffmpeg, the parts go beside the file, numbered.
pub async fn join(takes: Takes) -> Result<PathBuf, String> {
    let parts: Vec<PathBuf> = takes
        .parts
        .iter()
        .filter(|part| std::fs::metadata(part).is_ok_and(|file| file.len() > 0))
        .cloned()
        .collect();
    let result = match parts.as_slice() {
        [] => Err("the recorder wrote nothing".to_owned()),
        [one] => std::fs::rename(one, &takes.file)
            .map(|()| takes.file.clone())
            .map_err(|error| error.to_string()),
        _ if !mochi_core::process::installed("ffmpeg") => keep_apart(&takes, &parts),
        _ => {
            let sizes = sizes(&parts).await;
            let audio = has_audio(&parts[0]).await;
            let args = match sizes.first().copied().flatten() {
                Some(size) if sizes.iter().all(|other| *other == Some(size)) => {
                    let list = takes.dir.join("parts.txt");
                    std::fs::write(&list, concat_list(&parts))
                        .map_err(|error| error.to_string())?;
                    copy_args(&list, &takes.file)
                }
                Some(size) => encode_args(&parts, size, audio, &takes.file),
                None => encode_args(&parts, (1920, 1080), audio, &takes.file),
            };
            run_ffmpeg(&args).await.map(|()| takes.file.clone())
        }
    };
    if result.is_ok() {
        let _ = std::fs::remove_dir_all(&takes.dir);
    }
    result
}

/// Without ffmpeg: each part beside the recording, as "<name> part 2.mp4".
fn keep_apart(takes: &Takes, parts: &[PathBuf]) -> Result<PathBuf, String> {
    let stem = takes
        .file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = takes.file.parent().unwrap_or_else(|| Path::new("."));
    let mut first = None;
    for (index, part) in parts.iter().enumerate() {
        let name = format!("{stem} part {}.{}", index + 1, extension(&takes.file));
        let to = parent.join(name);
        std::fs::rename(part, &to).map_err(|error| error.to_string())?;
        first.get_or_insert(to);
    }
    let _ = std::fs::remove_dir_all(&takes.dir);
    first.ok_or_else(|| "the recorder wrote nothing".to_owned())
}

/// Each part's size, `None` where ffprobe can't tell.
async fn sizes(parts: &[PathBuf]) -> Vec<Option<(u32, u32)>> {
    let mut sizes = Vec::new();
    for part in parts {
        sizes.push(size(part).await);
    }
    sizes
}

/// A video's width and height, from ffprobe.
async fn size(file: &Path) -> Option<(u32, u32)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    parse_size(&String::from_utf8_lossy(&output.stdout))
}

fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.trim().split_once(',')?;
    Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
}

async fn has_audio(file: &Path) -> bool {
    Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a",
            "-show_entries",
            "stream=index",
            "-of",
            "csv=p=0",
        ])
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .await
        .is_ok_and(|output| !output.stdout.trim_ascii().is_empty())
}

/// ffmpeg's concat list: one `file '<path>'` a line.
fn concat_list(parts: &[PathBuf]) -> String {
    parts
        .iter()
        .map(|part| {
            format!(
                "file '{}'\n",
                part.display().to_string().replace('\'', "'\\''")
            )
        })
        .collect()
}

/// Parts of one size, joined as they are.
fn copy_args(list: &Path, out: &Path) -> Vec<String> {
    [
        "-nostdin",
        "-y",
        "-loglevel",
        "error",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain([list.display().to_string()])
    .chain(["-c", "copy"].map(str::to_owned))
    .chain([out.display().to_string()])
    .collect()
}

/// Parts of different sizes: each scaled into `size`, centered on black,
/// then joined, sound and all.
fn encode_args(
    parts: &[PathBuf],
    (width, height): (u32, u32),
    audio: bool,
    out: &Path,
) -> Vec<String> {
    let mut args: Vec<String> = ["-nostdin", "-y", "-loglevel", "error"]
        .map(str::to_owned)
        .into();
    for part in parts {
        args.push("-i".into());
        args.push(part.display().to_string());
    }
    let mut filter = String::new();
    let mut joined = String::new();
    for index in 0..parts.len() {
        filter.push_str(&format!(
            "[{index}:v]scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1[v{index}];"
        ));
        joined.push_str(&format!("[v{index}]"));
        if audio {
            joined.push_str(&format!("[{index}:a]"));
        }
    }
    let streams = if audio { "[v][a]" } else { "[v]" };
    filter.push_str(&format!(
        "{joined}concat=n={}:v=1:a={}{streams}",
        parts.len(),
        u8::from(audio)
    ));
    args.extend([
        "-filter_complex".to_owned(),
        filter,
        "-map".to_owned(),
        "[v]".to_owned(),
    ]);
    if audio {
        args.extend(["-map".to_owned(), "[a]".to_owned()]);
    }
    args.push(out.display().to_string());
    args
}

async fn run_ffmpeg(args: &[String]) -> Result<(), String> {
    let output = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("can't run ffmpeg: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let said = String::from_utf8_lossy(&output.stderr);
        let last = said
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        Err(format!(
            "couldn't join the recording's parts: {}",
            last.trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_go_in_a_scratch_folder() {
        let dir = std::env::temp_dir().join(format!("mochi-takes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Recording.mkv");
        std::fs::write(&file, "first").unwrap();
        let mut takes = Takes::begin(&file).unwrap();
        assert!(!file.exists());
        assert_eq!(takes.parts, [dir.join(".mochi-parts-Recording/0.mkv")]);
        assert_eq!(takes.next(), dir.join(".mochi-parts-Recording/1.mkv"));
        takes.parts.push(takes.next());
        assert_eq!(takes.next(), dir.join(".mochi-parts-Recording/2.mkv"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ffmpeg_joins_by_copy_or_again() {
        assert_eq!(parse_size("1920,1080\n"), Some((1920, 1080)));
        assert_eq!(parse_size(""), None);
        let parts = [PathBuf::from("/a/0.mp4"), PathBuf::from("/a/it's.mp4")];
        assert_eq!(
            concat_list(&parts),
            "file '/a/0.mp4'\nfile '/a/it'\\''s.mp4'\n"
        );
        let copy = copy_args(Path::new("/a/parts.txt"), Path::new("/v/R.mp4"));
        assert_eq!(
            copy.join(" "),
            "-nostdin -y -loglevel error -f concat -safe 0 -i /a/parts.txt -c copy /v/R.mp4"
        );
        let encode = encode_args(&parts, (1920, 1080), true, Path::new("/v/R.mp4"));
        let filter = &encode[encode
            .iter()
            .position(|arg| arg == "-filter_complex")
            .unwrap()
            + 1];
        assert!(
            filter.ends_with("[v0][0:a][v1][1:a]concat=n=2:v=1:a=1[v][a]"),
            "{filter}"
        );
        assert!(encode.ends_with(&[
            "-map".into(),
            "[v]".into(),
            "-map".into(),
            "[a]".into(),
            "/v/R.mp4".into()
        ]));
    }

    /// Two screens of different sizes, then of one: needs ffmpeg, and
    /// passes without it.
    #[tokio::test]
    async fn parts_join_into_one_video() {
        if !mochi_core::process::installed("ffmpeg") {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mochi-join-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let clip = |name: &str, size: &str| {
            let out = dir.join(name);
            let made = std::process::Command::new("ffmpeg")
                .args(["-nostdin", "-loglevel", "error", "-f", "lavfi", "-i"])
                .arg(format!("testsrc=size={size}:rate=10:duration=1"))
                .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
                .arg(&out)
                .status()
                .unwrap();
            assert!(made.success());
            out
        };
        for (sizes, joined) in [
            (["320x240", "200x200"], "Mixed.mp4"),
            (["320x240", "320x240"], "Same.mp4"),
        ] {
            let file = dir.join(joined);
            clip(joined, sizes[0]);
            let mut takes = Takes::begin(&file).unwrap();
            let next = takes.next();
            std::fs::rename(clip("next.mp4", sizes[1]), &next).unwrap();
            takes.parts.push(next);
            let parts = takes.dir.clone();
            assert_eq!(join(takes).await, Ok(file.clone()));
            assert_eq!(size(&file).await, Some((320, 240)));
            assert!(!parts.exists());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

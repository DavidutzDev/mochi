//! What was dropped: the files, from the URIs the island hands over, and
//! what kind each is.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Image,
    Pdf,
    Archive,
    Folder,
    Other,
}

impl Kind {
    pub fn of(path: &Path) -> Self {
        if path.is_dir() {
            return Self::Folder;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
        match extension {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff" | "avif" | "heic" => {
                Self::Image
            }
            "pdf" => Self::Pdf,
            "zip" | "tar" | "tgz" | "txz" | "tbz2" | "7z" | "rar" => Self::Archive,
            "gz" | "xz" | "zst" | "bz2" if name.contains(".tar.") => Self::Archive,
            _ => Self::Other,
        }
    }

    fn noun(self, count: usize) -> &'static str {
        let one = count == 1;
        match self {
            Self::Image => {
                if one {
                    "image"
                } else {
                    "images"
                }
            }
            Self::Pdf => {
                if one {
                    "PDF"
                } else {
                    "PDFs"
                }
            }
            Self::Archive => {
                if one {
                    "archive"
                } else {
                    "archives"
                }
            }
            Self::Folder => {
                if one {
                    "folder"
                } else {
                    "folders"
                }
            }
            Self::Other => {
                if one {
                    "file"
                } else {
                    "files"
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub path: PathBuf,
    pub kind: Kind,
}

/// The files in what the island sends: `file://` URIs or paths, one a
/// line. Other URIs, like a link from a browser, and files that don't exist
/// are left out.
pub fn parse(text: &str) -> Vec<Dropped> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let path = match line.strip_prefix("file://") {
                // A host part, like file://localhost/home, is this machine.
                Some(rest) => PathBuf::from(decode(&rest[rest.find('/')?..])),
                None if line.starts_with('/') => PathBuf::from(line),
                None => return None,
            };
            path.exists().then(|| Dropped {
                kind: Kind::of(&path),
                path,
            })
        })
        .collect()
}

/// Undoes a URI's percent-encoding.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(byte) = text
                .get(index + 1..index + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What was dropped, in words: "report.pdf", "3 images", "2 PDFs and a
/// folder".
pub fn summary(files: &[Dropped]) -> String {
    if let [one] = files {
        return one
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    let mut parts: Vec<String> = Vec::new();
    for kind in [
        Kind::Image,
        Kind::Pdf,
        Kind::Archive,
        Kind::Folder,
        Kind::Other,
    ] {
        let count = files.iter().filter(|file| file.kind == kind).count();
        match count {
            0 => {}
            1 => parts.push(format!(
                "{} {}",
                if kind == Kind::Image || kind == Kind::Archive {
                    "an"
                } else {
                    "a"
                },
                kind.noun(1)
            )),
            _ => parts.push(format!("{count} {}", kind.noun(count))),
        }
    }
    match parts.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// A path for something new next to `beside`: `name`, or `name 2`,
/// `name 3` before the extension while one exists.
pub fn unique(directory: &Path, stem: &str, extension: &str) -> PathBuf {
    let dotted = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    let first = directory.join(format!("{stem}{dotted}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|number| directory.join(format!("{stem} {number}{dotted}")))
        .find(|path| !path.exists())
        .expect("some number is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mochi-drop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn files_come_from_uris_and_paths() {
        let dir = scratch("parse");
        std::fs::write(dir.join("my report.pdf"), "").unwrap();
        std::fs::write(dir.join("photo.JPG"), "").unwrap();
        std::fs::write(dir.join("backup.tar.zst"), "").unwrap();
        let text = format!(
            "file://{0}/my%20report.pdf\nfile://localhost{0}/photo.JPG\n{0}/backup.tar.zst\nhttps://example.com\nfile://{0}/missing.txt\n",
            dir.display()
        );
        let files = parse(&text);
        let kinds: Vec<Kind> = files.iter().map(|file| file.kind).collect();
        assert_eq!(kinds, [Kind::Pdf, Kind::Image, Kind::Archive]);
        assert_eq!(files[0].path, dir.join("my report.pdf"));
        assert_eq!(parse(&format!("{}\n", dir.display()))[0].kind, Kind::Folder);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn summaries_count_each_kind() {
        let file = |name: &str, kind| Dropped {
            path: PathBuf::from(format!("/tmp/{name}")),
            kind,
        };
        assert_eq!(summary(&[file("a.pdf", Kind::Pdf)]), "a.pdf");
        assert_eq!(
            summary(&[file("a.png", Kind::Image), file("b.png", Kind::Image)]),
            "2 images"
        );
        assert_eq!(
            summary(&[
                file("a.pdf", Kind::Pdf),
                file("b.pdf", Kind::Pdf),
                file("c", Kind::Folder),
                file("d.zip", Kind::Archive)
            ]),
            "2 PDFs, an archive and a folder"
        );
    }

    #[test]
    fn new_files_never_replace_old_ones() {
        let dir = scratch("unique");
        assert_eq!(unique(&dir, "Archive", "zip"), dir.join("Archive.zip"));
        std::fs::write(dir.join("Archive.zip"), "").unwrap();
        std::fs::write(dir.join("Archive 2.zip"), "").unwrap();
        assert_eq!(unique(&dir, "Archive", "zip"), dir.join("Archive 3.zip"));
        assert_eq!(unique(&dir, "photos", ""), dir.join("photos"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

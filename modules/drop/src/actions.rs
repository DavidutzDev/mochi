//! What can be done with dropped files, and how. Each action shows only
//! when it fits what was dropped and what it needs is there: images
//! convert in Mochi itself, other files through a program, like ffmpeg for
//! videos and sound or LibreOffice for documents. New files go next to the
//! first dropped one, under a name that's free.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::files::{Dropped, Kind, unique};

/// An action to offer: a button, or a format under "Convert to".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub icon: &'static str,
    /// A conversion, shown with the formats.
    pub convert: bool,
}

/// The actions the settings name, in the order they're offered.
/// `convert` stands for every conversion.
pub const IDS: [&str; 6] = ["zip", "extract", "merge", "convert", "copy", "open"];

/// One thing to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Run a program in a folder.
    Run {
        program: String,
        args: Vec<OsString>,
        cwd: PathBuf,
    },
    /// Make a folder, for extracting into.
    MakeDir(PathBuf),
    /// Move a file, for a program that names its output itself.
    Move { from: PathBuf, to: PathBuf },
    /// Convert an image in Mochi itself, to the format `output`'s
    /// extension says.
    Image { input: PathBuf, output: PathBuf },
}

impl Step {
    fn run(program: &str, args: Vec<OsString>, cwd: &Path) -> Self {
        Self::Run {
            program: program.to_owned(),
            args,
            cwd: cwd.to_owned(),
        }
    }
}

/// What an action does: its steps, then what to say and where the result
/// is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub done: String,
    pub result: Option<PathBuf>,
}

/// A format files convert to.
struct Format {
    /// Its extension, which is also the action's id after `to-`.
    extension: &'static str,
    label: &'static str,
    /// What converts to it.
    from: &'static [Kind],
}

const FORMATS: [Format; 20] = [
    Format {
        extension: "png",
        label: "PNG",
        from: &[Kind::Image],
    },
    Format {
        extension: "jpg",
        label: "JPEG",
        from: &[Kind::Image],
    },
    Format {
        extension: "webp",
        label: "WebP",
        from: &[Kind::Image],
    },
    Format {
        extension: "gif",
        label: "GIF",
        from: &[Kind::Image, Kind::Video],
    },
    Format {
        extension: "bmp",
        label: "BMP",
        from: &[Kind::Image],
    },
    Format {
        extension: "tiff",
        label: "TIFF",
        from: &[Kind::Image],
    },
    Format {
        extension: "ico",
        label: "ICO",
        from: &[Kind::Image],
    },
    Format {
        extension: "svg",
        label: "SVG",
        from: &[Kind::Image],
    },
    Format {
        extension: "mp4",
        label: "MP4",
        from: &[Kind::Video],
    },
    Format {
        extension: "webm",
        label: "WebM",
        from: &[Kind::Video],
    },
    Format {
        extension: "mkv",
        label: "MKV",
        from: &[Kind::Video],
    },
    Format {
        extension: "mp3",
        label: "MP3",
        from: &[Kind::Video, Kind::Audio],
    },
    Format {
        extension: "ogg",
        label: "Ogg",
        from: &[Kind::Audio],
    },
    Format {
        extension: "flac",
        label: "FLAC",
        from: &[Kind::Audio],
    },
    Format {
        extension: "wav",
        label: "WAV",
        from: &[Kind::Audio],
    },
    Format {
        extension: "pdf",
        label: "PDF",
        from: &[Kind::Image, Kind::Document, Kind::Text],
    },
    Format {
        extension: "docx",
        label: "Word",
        from: &[Kind::Document, Kind::Text],
    },
    Format {
        extension: "odt",
        label: "ODT",
        from: &[Kind::Document, Kind::Text],
    },
    Format {
        extension: "html",
        label: "HTML",
        from: &[Kind::Document, Kind::Text],
    },
    Format {
        extension: "md",
        label: "Markdown",
        from: &[Kind::Document, Kind::Text],
    },
];

/// Image formats Mochi reads and writes itself.
const NATIVE_READ: [&str; 8] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff"];
const NATIVE_WRITE: [&str; 7] = ["png", "jpg", "webp", "gif", "bmp", "tiff", "ico"];

fn extension(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// The same format under another spelling, like jpeg and jpg.
fn same_format(extension: &str, target: &str) -> bool {
    let normal = |extension: &str| {
        match extension {
            "jpeg" => "jpg",
            "tif" => "tiff",
            "htm" => "html",
            "markdown" => "md",
            other => other,
        }
        .to_owned()
    };
    normal(extension) == normal(target)
}

/// The first program of `programs` that's installed.
fn pick<'a>(programs: &[&'a str], installed: &impl Fn(&str) -> bool) -> Option<&'a str> {
    programs.iter().copied().find(|program| installed(program))
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn os(path: &Path) -> OsString {
    path.as_os_str().to_owned()
}

/// The actions that fit `files`, among `enabled`, with what's installed.
pub fn offered(
    files: &[Dropped],
    enabled: &[String],
    installed: &impl Fn(&str) -> bool,
) -> Vec<Action> {
    let wants = |id: &str| enabled.iter().any(|enabled| enabled == id);
    let button = |id: &str, label: &str, icon| Action {
        id: id.to_owned(),
        label: label.to_owned(),
        icon,
        convert: false,
    };
    let mut actions = Vec::new();
    for (id, label, icon) in [
        ("zip", "Compress", "folder_zip"),
        ("extract", "Extract", "unarchive"),
        ("merge", "Merge PDFs", "picture_as_pdf"),
    ] {
        if wants(id) && plan(id, files, installed).is_ok() {
            actions.push(button(id, label, icon));
        }
    }
    if wants("convert") {
        for format in &FORMATS {
            let id = format!("to-{}", format.extension);
            if plan(&id, files, installed).is_ok() {
                actions.push(Action {
                    id,
                    label: format.label.to_owned(),
                    icon: "transform",
                    convert: true,
                });
            }
        }
    }
    for (id, label, icon) in [
        ("copy", "Copy paths", "content_copy"),
        ("open", "Open", "open_in_new"),
    ] {
        if wants(id) && plan(id, files, installed).is_ok() {
            actions.push(button(id, label, icon));
        }
    }
    actions
}

/// How to do `action` to `files`, or why it can't be.
pub fn plan(
    action: &str,
    files: &[Dropped],
    installed: &impl Fn(&str) -> bool,
) -> Result<Plan, String> {
    let first = files.first().ok_or("nothing was dropped")?;
    let parent = first
        .path
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .to_path_buf();
    let missing = |what: &str| format!("{what} needs a program that isn't installed");
    if let Some(target) = action.strip_prefix("to-") {
        return convert(target, files, installed);
    }
    match action {
        "zip" => {
            let stem = match files {
                [one] => stem(&one.path),
                _ => "Archive".to_owned(),
            };
            let out = unique(&parent, &stem, "zip");
            // Names relative to the folder they share, so the archive holds
            // them without their whole path.
            let together = files.iter().all(|file| file.path.parent() == Some(&parent));
            let names = files.iter().map(|file| match file.path.file_name() {
                Some(name) if together => name.to_os_string(),
                _ => os(&file.path),
            });
            let program =
                pick(&["zip", "bsdtar"], installed).ok_or_else(|| missing("Compressing"))?;
            let head: [OsString; 3] = if program == "zip" {
                ["-r".into(), "-q".into(), os(&out)]
            } else {
                ["-a".into(), "-cf".into(), os(&out)]
            };
            Ok(Plan {
                steps: vec![Step::run(
                    program,
                    head.into_iter().chain(names).collect(),
                    &parent,
                )],
                done: format!("Saved {}", name(&out)),
                result: Some(out),
            })
        }
        "extract" => {
            let archives: Vec<&Dropped> = files
                .iter()
                .filter(|file| file.kind == Kind::Archive)
                .collect();
            if archives.is_empty() {
                return Err("no archive was dropped".into());
            }
            let mut steps = Vec::new();
            let mut last = None;
            for archive in &archives {
                let name = archive
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // backup.tar.gz → backup
                let stem = name
                    .split_once(".tar.")
                    .map(|(stem, _)| stem.to_owned())
                    .unwrap_or_else(|| stem(&archive.path));
                let directory = archive.path.parent().unwrap_or(&parent).to_path_buf();
                let into = unique(&directory, &stem, "");
                let kind = extension(&archive.path);
                let program = if installed("bsdtar") {
                    "bsdtar"
                } else if kind == "zip" && installed("unzip") {
                    "unzip"
                } else if (kind == "7z" || kind == "rar") && installed("7z") {
                    "7z"
                } else if !matches!(kind.as_str(), "zip" | "7z" | "rar") && installed("tar") {
                    "tar"
                } else {
                    return Err(missing("Extracting"));
                };
                let (path, target) = (os(&archive.path), os(&into));
                let args: Vec<OsString> = match program {
                    "unzip" => vec!["-q".into(), path, "-d".into(), target],
                    "7z" => {
                        let mut out = OsString::from("-o");
                        out.push(&target);
                        vec!["x".into(), "-y".into(), out, path]
                    }
                    _ => vec!["-xf".into(), path, "-C".into(), target],
                };
                steps.push(Step::MakeDir(into.clone()));
                steps.push(Step::run(program, args, &directory));
                last = Some(into);
            }
            Ok(Plan {
                done: match (archives.len(), &last) {
                    (1, Some(into)) => format!("Extracted to {}", name(into)),
                    (count, _) => format!("Extracted {count} archives"),
                },
                steps,
                result: last,
            })
        }
        "merge" => {
            let pdfs: Vec<&Dropped> = files.iter().filter(|file| file.kind == Kind::Pdf).collect();
            if pdfs.len() < 2 {
                return Err("merging takes two PDFs or more".into());
            }
            let out = unique(&parent, "Merged", "pdf");
            let inputs = pdfs.iter().map(|pdf| os(&pdf.path));
            let program =
                pick(&["pdfunite", "qpdf"], installed).ok_or_else(|| missing("Merging"))?;
            let args: Vec<OsString> = if program == "pdfunite" {
                inputs.chain([os(&out)]).collect()
            } else {
                [OsString::from("--empty"), "--pages".into()]
                    .into_iter()
                    .chain(inputs)
                    .chain(["--".into(), os(&out)])
                    .collect()
            };
            Ok(Plan {
                steps: vec![Step::run(program, args, &parent)],
                done: format!("Saved {}", name(&out)),
                result: Some(out),
            })
        }
        "copy" => {
            if !installed("wl-copy") {
                return Err(missing("Copying"));
            }
            let text: Vec<String> = files
                .iter()
                .map(|file| file.path.display().to_string())
                .collect();
            Ok(Plan {
                steps: vec![Step::run(
                    "wl-copy",
                    vec!["--".into(), text.join("\n").into()],
                    &parent,
                )],
                done: if files.len() == 1 {
                    "Copied the path".into()
                } else {
                    format!("Copied {} paths", files.len())
                },
                result: None,
            })
        }
        "open" => {
            if !installed("xdg-open") {
                return Err(missing("Opening"));
            }
            Ok(Plan {
                steps: files
                    .iter()
                    .take(10)
                    .map(|file| Step::run("xdg-open", vec![os(&file.path)], &parent))
                    .collect(),
                done: "Opened".into(),
                result: None,
            })
        }
        other => Err(format!("no action {other}")),
    }
}

/// Converts every dropped file that can become `target` and isn't one
/// already.
fn convert(
    target: &str,
    files: &[Dropped],
    installed: &impl Fn(&str) -> bool,
) -> Result<Plan, String> {
    let format = FORMATS
        .iter()
        .find(|format| format.extension == target)
        .ok_or_else(|| format!("no format {target}"))?;
    let inputs: Vec<&Dropped> = files
        .iter()
        .filter(|file| {
            format.from.contains(&file.kind) && !same_format(&extension(&file.path), target)
        })
        .collect();
    if inputs.is_empty() {
        return Err(format!("nothing dropped converts to {}", format.label));
    }
    let mut steps = Vec::new();
    let mut last = None;
    for file in &inputs {
        let directory = file
            .path
            .parent()
            .unwrap_or_else(|| Path::new("/"))
            .to_path_buf();
        let out = unique(&directory, &stem(&file.path), target);
        steps.extend(convert_one(file, &out, target, installed)?);
        last = Some(out);
    }
    Ok(Plan {
        done: match (inputs.len(), &last) {
            (1, Some(out)) => format!("Saved {}", name(out)),
            (count, _) => format!("Converted {count} files to {}", format.label),
        },
        steps,
        result: last,
    })
}

/// The steps that make `out` from `file`.
fn convert_one(
    file: &Dropped,
    out: &Path,
    target: &str,
    installed: &impl Fn(&str) -> bool,
) -> Result<Vec<Step>, String> {
    let source = extension(&file.path);
    let directory = out.parent().unwrap_or_else(|| Path::new("/")).to_path_buf();
    let needs = |what: &str| format!("converting to {target} needs {what}");
    let input = os(&file.path);
    let ffmpeg = |extra: &[&str]| -> Result<Vec<Step>, String> {
        if !installed("ffmpeg") {
            return Err(needs("ffmpeg"));
        }
        let args = ["-nostdin", "-loglevel", "error", "-i"]
            .into_iter()
            .map(OsString::from)
            .chain([input.clone()])
            .chain(extra.iter().map(OsString::from))
            .chain([os(out)])
            .collect();
        Ok(vec![Step::run("ffmpeg", args, &directory)])
    };
    match (file.kind, target) {
        (Kind::Image, "svg") => {
            if !installed("vtracer") {
                return Err(needs("vtracer"));
            }
            Ok(vec![Step::run(
                "vtracer",
                vec!["--input".into(), input, "--output".into(), os(out)],
                &directory,
            )])
        }
        (Kind::Image, _)
            if NATIVE_READ.contains(&source.as_str()) && NATIVE_WRITE.contains(&target) =>
        {
            Ok(vec![Step::Image {
                input: file.path.clone(),
                output: out.to_owned(),
            }])
        }
        (Kind::Image, _) => {
            let program =
                pick(&["magick", "convert"], installed).ok_or_else(|| needs("ImageMagick"))?;
            Ok(vec![Step::run(program, vec![input, os(out)], &directory)])
        }
        (Kind::Video, "gif") => ffmpeg(&["-vf", "fps=12,scale=480:-1:flags=lanczos"]),
        (Kind::Video, "mp3") => ffmpeg(&["-vn"]),
        (Kind::Video | Kind::Audio, _) => ffmpeg(&[]),
        // Office files convert through LibreOffice, Markdown and HTML
        // through pandoc; a PDF from those goes through LibreOffice too.
        (Kind::Document, "pdf" | "docx" | "odt" | "html") | (Kind::Text, "pdf") => {
            let program =
                pick(&["soffice", "libreoffice"], installed).ok_or_else(|| needs("LibreOffice"))?;
            // It names the file itself, in the folder it's given: a
            // folder of its own, then the free name.
            let temporary = directory.join(format!(".mochi-convert-{}", std::process::id()));
            Ok(vec![
                Step::MakeDir(temporary.clone()),
                Step::run(
                    program,
                    vec![
                        "--headless".into(),
                        "--convert-to".into(),
                        target.into(),
                        "--outdir".into(),
                        os(&temporary),
                        input,
                    ],
                    &directory,
                ),
                Step::Move {
                    from: temporary.join(format!("{}.{target}", stem(&file.path))),
                    to: out.to_owned(),
                },
            ])
        }
        (Kind::Document | Kind::Text, _) => {
            if !installed("pandoc") {
                return Err(needs("pandoc"));
            }
            Ok(vec![Step::run(
                "pandoc",
                vec![input, "-o".into(), os(out)],
                &directory,
            )])
        }
        _ => Err(format!("{} doesn't convert to {target}", name(&file.path))),
    }
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Converts an image in Mochi itself: decodes it, then writes the format
/// `output`'s extension names. JPEG has no transparency, so it goes on
/// white; an icon is at most 256 pixels a side.
pub fn convert_image(input: &Path, output: &Path) -> Result<(), String> {
    let image = image::ImageReader::open(input)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|error| format!("can't read {}: {error}", name(input)))?
        .decode()
        .map_err(|error| format!("can't read {}: {error}", name(input)))?;
    let image = match extension(output).as_str() {
        "jpg" | "jpeg" => {
            let mut flat = image::RgbImage::new(image.width(), image.height());
            for (x, y, pixel) in image.to_rgba8().enumerate_pixels() {
                let alpha = f32::from(pixel[3]) / 255.0;
                let blend = |channel: u8| {
                    (f32::from(channel) * alpha + 255.0 * (1.0 - alpha)).round() as u8
                };
                flat.put_pixel(
                    x,
                    y,
                    image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]),
                );
            }
            image::DynamicImage::ImageRgb8(flat)
        }
        "ico" if image.width() > 256 || image.height() > 256 => image.thumbnail(256, 256),
        _ => image,
    };
    image
        .save(output)
        .map_err(|error| format!("can't write {}: {error}", name(output)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, kind: Kind) -> Dropped {
        Dropped {
            path: PathBuf::from(path),
            kind,
        }
    }

    fn ids(actions: &[Action]) -> Vec<String> {
        actions.iter().map(|action| action.id.clone()).collect()
    }

    fn all() -> Vec<String> {
        IDS.iter().map(|id| (*id).to_owned()).collect()
    }

    fn args(step: &Step) -> Vec<String> {
        match step {
            Step::Run { args, .. } => args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
            other => panic!("not a program: {other:?}"),
        }
    }

    #[test]
    fn images_convert_without_any_program() {
        let nothing = |_: &str| false;
        let webp = [file("/nowhere/cat.webp", Kind::Image)];
        let found = ids(&offered(&webp, &all(), &nothing));
        assert_eq!(
            found,
            ["to-png", "to-jpg", "to-gif", "to-bmp", "to-tiff", "to-ico"]
        );
        let converted = plan("to-png", &webp, &nothing).unwrap();
        assert_eq!(
            converted.steps,
            [Step::Image {
                input: "/nowhere/cat.webp".into(),
                output: "/nowhere/cat.png".into()
            }]
        );
        // With the programs, SVG and PDF too.
        let everything = |_: &str| true;
        let found = ids(&offered(&webp, &all(), &everything));
        assert!(found.contains(&"to-svg".to_owned()) && found.contains(&"to-pdf".to_owned()));
        // HEIC needs ImageMagick to read.
        let heic = [file("/nowhere/photo.heic", Kind::Image)];
        assert!(plan("to-jpg", &heic, &nothing).is_err());
        let convert = plan("to-jpg", &heic, &|program: &str| program == "magick").unwrap();
        assert_eq!(
            args(&convert.steps[0]),
            ["/nowhere/photo.heic", "/nowhere/photo.jpg"]
        );
    }

    #[test]
    fn other_files_convert_through_their_programs() {
        let everything = |_: &str| true;
        let video = [file("/nowhere/clip.mkv", Kind::Video)];
        let found = ids(&offered(&video, &all(), &everything));
        assert!(found.contains(&"to-gif".to_owned()) && found.contains(&"to-mp3".to_owned()));
        assert!(!found.contains(&"to-mkv".to_owned()));
        let mp3 = plan("to-mp3", &video, &everything).unwrap();
        assert_eq!(
            args(&mp3.steps[0]),
            [
                "-nostdin",
                "-loglevel",
                "error",
                "-i",
                "/nowhere/clip.mkv",
                "-vn",
                "/nowhere/clip.mp3"
            ]
        );

        let doc = [file("/nowhere/report.docx", Kind::Document)];
        let pdf = plan("to-pdf", &doc, &|program: &str| program == "soffice").unwrap();
        assert!(matches!(pdf.steps[0], Step::MakeDir(_)));
        assert!(
            matches!(&pdf.steps[2], Step::Move { to, .. } if to == Path::new("/nowhere/report.pdf"))
        );
        let md = plan("to-md", &doc, &|program: &str| program == "pandoc").unwrap();
        assert_eq!(
            args(&md.steps[0]),
            ["/nowhere/report.docx", "-o", "/nowhere/report.md"]
        );
        // Nothing converts a folder.
        let folder = [file("/nowhere/stuff", Kind::Folder)];
        assert!(
            offered(&folder, &all(), &everything)
                .iter()
                .all(|action| !action.convert)
        );
    }

    #[test]
    fn actions_fit_what_was_dropped() {
        let everything = |_: &str| true;
        let pdfs = [
            file("/nowhere/a.pdf", Kind::Pdf),
            file("/nowhere/b.pdf", Kind::Pdf),
        ];
        assert_eq!(
            ids(&offered(&pdfs, &all(), &everything)),
            ["zip", "merge", "copy", "open"]
        );
        let archive = [file("/nowhere/backup.tar.gz", Kind::Archive)];
        assert!(ids(&offered(&archive, &all(), &everything)).contains(&"extract".to_owned()));
        // The settings pick.
        assert_eq!(
            ids(&offered(&pdfs, &["merge".to_owned()], &everything)),
            ["merge"]
        );
    }

    #[test]
    fn plans_name_their_programs_and_results() {
        let only = |name: &'static str| move |program: &str| program == name;
        let pdfs = [
            file("/nowhere/a.pdf", Kind::Pdf),
            file("/nowhere/b.pdf", Kind::Pdf),
        ];
        let merge = plan("merge", &pdfs, &only("pdfunite")).unwrap();
        assert_eq!(
            args(&merge.steps[0]),
            ["/nowhere/a.pdf", "/nowhere/b.pdf", "/nowhere/Merged.pdf"]
        );
        let zip = plan("zip", &pdfs, &only("zip")).unwrap();
        assert_eq!(
            args(&zip.steps[0]),
            ["-r", "-q", "/nowhere/Archive.zip", "a.pdf", "b.pdf"]
        );
        assert_eq!(zip.done, "Saved Archive.zip");

        let archive = [file("/nowhere/backup.tar.gz", Kind::Archive)];
        let extract = plan("extract", &archive, &only("tar")).unwrap();
        assert_eq!(extract.steps[0], Step::MakeDir("/nowhere/backup".into()));
        assert_eq!(
            args(&extract.steps[1]),
            ["-xf", "/nowhere/backup.tar.gz", "-C", "/nowhere/backup"]
        );
        assert_eq!(extract.done, "Extracted to backup");
    }

    #[test]
    fn images_really_convert() {
        let dir = std::env::temp_dir().join(format!("mochi-drop-image-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut source = image::RgbaImage::new(300, 200);
        source.put_pixel(0, 0, image::Rgba([255, 0, 0, 0]));
        let webp = dir.join("cat.webp");
        image::DynamicImage::ImageRgba8(source).save(&webp).unwrap();
        for target in NATIVE_WRITE {
            let out = dir.join(format!("cat.{target}"));
            convert_image(&webp, &out).unwrap();
            let back = image::open(&out).unwrap();
            assert!(back.width() > 0, "{target}");
        }
        // A transparent pixel lands on white in a JPEG; an icon shrinks.
        let jpeg = image::open(dir.join("cat.jpg")).unwrap().to_rgb8();
        assert!(jpeg.get_pixel(0, 0)[1] > 200);
        assert!(image::open(dir.join("cat.ico")).unwrap().width() <= 256);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

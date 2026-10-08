//! What can be done with dropped files, and the programs that do it. Each
//! action shows only when it fits what was dropped and a program for it is
//! installed. New files go next to the first dropped one, under a name
//! that's free.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::files::{Dropped, Kind, unique};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
}

/// Every action, in the order they're offered.
pub const ALL: [Action; 8] = [
    Action {
        id: "zip",
        label: "Compress",
        icon: "folder_zip",
    },
    Action {
        id: "extract",
        label: "Extract",
        icon: "unarchive",
    },
    Action {
        id: "merge",
        label: "Merge PDFs",
        icon: "picture_as_pdf",
    },
    Action {
        id: "png",
        label: "To PNG",
        icon: "image",
    },
    Action {
        id: "jpg",
        label: "To JPEG",
        icon: "image",
    },
    Action {
        id: "webp",
        label: "To WebP",
        icon: "image",
    },
    Action {
        id: "copy",
        label: "Copy paths",
        icon: "content_copy",
    },
    Action {
        id: "open",
        label: "Open",
        icon: "open_in_new",
    },
];

/// One program to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub program: String,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    /// A folder to make first, for extracting into.
    pub make: Option<PathBuf>,
}

/// What an action does: programs to run, then what to say and where the
/// result is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub done: String,
    pub result: Option<PathBuf>,
}

fn of_kind(files: &[Dropped], kind: Kind) -> Vec<&Dropped> {
    files.iter().filter(|file| file.kind == kind).collect()
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// The first program of `programs` that's installed.
fn pick<'a>(programs: &[&'a str], installed: &impl Fn(&str) -> bool) -> Option<&'a str> {
    programs.iter().copied().find(|program| installed(program))
}

/// The actions that fit `files`, among `enabled`, with what's installed.
pub fn offered(
    files: &[Dropped],
    enabled: &[String],
    installed: &impl Fn(&str) -> bool,
) -> Vec<Action> {
    ALL.into_iter()
        .filter(|action| enabled.iter().any(|id| id == action.id))
        .filter(|action| plan(action.id, files, installed).is_ok())
        .collect()
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
    match action {
        "zip" => {
            let stem = match files {
                [one] => one
                    .path
                    .file_stem()
                    .map_or("Archive".into(), |stem| stem.to_string_lossy().into_owned()),
                _ => "Archive".to_owned(),
            };
            let out = unique(&parent, &stem, "zip");
            // Names relative to the folder they share, so the archive holds
            // them without their whole path.
            let together = files.iter().all(|file| file.path.parent() == Some(&parent));
            let names: Vec<OsString> = files
                .iter()
                .map(|file| match file.path.file_name() {
                    Some(name) if together => name.to_os_string(),
                    _ => file.path.clone().into_os_string(),
                })
                .collect();
            let step =
                match pick(&["zip", "bsdtar"], installed).ok_or_else(|| missing("Compressing"))? {
                    "zip" => Step {
                        program: "zip".into(),
                        args: [
                            OsString::from("-r"),
                            "-q".into(),
                            out.clone().into_os_string(),
                        ]
                        .into_iter()
                        .chain(names)
                        .collect(),
                        cwd: parent,
                        make: None,
                    },
                    program => Step {
                        program: program.into(),
                        args: [
                            OsString::from("-a"),
                            "-cf".into(),
                            out.clone().into_os_string(),
                        ]
                        .into_iter()
                        .chain(names)
                        .collect(),
                        cwd: parent,
                        make: None,
                    },
                };
            Ok(Plan {
                steps: vec![step],
                done: format!("Saved {}", name(&out)),
                result: Some(out),
            })
        }
        "extract" => {
            let archives = of_kind(files, Kind::Archive);
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
                    .unwrap_or_else(|| {
                        name.rsplit_once('.')
                            .map_or(name.clone(), |(stem, _)| stem.to_owned())
                    });
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
                let path = archive.path.clone().into_os_string();
                let target = into.clone().into_os_string();
                let args: Vec<OsString> = match program {
                    "unzip" => vec!["-q".into(), path, "-d".into(), target],
                    "7z" => {
                        let mut out = OsString::from("-o");
                        out.push(&target);
                        vec!["x".into(), "-y".into(), out, path]
                    }
                    _ => vec!["-xf".into(), path, "-C".into(), target],
                };
                steps.push(Step {
                    program: program.into(),
                    args,
                    cwd: directory,
                    make: Some(into.clone()),
                });
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
            let pdfs = of_kind(files, Kind::Pdf);
            if pdfs.len() < 2 {
                return Err("merging takes two PDFs or more".into());
            }
            let out = unique(&parent, "Merged", "pdf");
            let inputs = pdfs.iter().map(|pdf| pdf.path.clone().into_os_string());
            let args: Vec<OsString> =
                match pick(&["pdfunite", "qpdf"], installed).ok_or_else(|| missing("Merging"))? {
                    "pdfunite" => inputs.chain([out.clone().into_os_string()]).collect(),
                    _ => [OsString::from("--empty"), "--pages".into()]
                        .into_iter()
                        .chain(inputs)
                        .chain(["--".into(), out.clone().into_os_string()])
                        .collect(),
                };
            let program = pick(&["pdfunite", "qpdf"], installed).unwrap_or("qpdf");
            Ok(Plan {
                steps: vec![Step {
                    program: program.into(),
                    args,
                    cwd: parent,
                    make: None,
                }],
                done: format!("Saved {}", name(&out)),
                result: Some(out),
            })
        }
        "png" | "jpg" | "webp" => {
            let wanted = |extension: &str| match action {
                "jpg" => extension == "jpg" || extension == "jpeg",
                other => extension == other,
            };
            let images: Vec<&Dropped> = of_kind(files, Kind::Image)
                .into_iter()
                .filter(|image| !wanted(&extension(&image.path)))
                .collect();
            if images.is_empty() {
                return Err("no image to convert".into());
            }
            let program =
                pick(&["magick", "convert"], installed).ok_or_else(|| missing("Converting"))?;
            let mut last = None;
            let steps = images
                .iter()
                .map(|image| {
                    let directory = image.path.parent().unwrap_or(&parent).to_path_buf();
                    let stem = image
                        .path
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let out = unique(&directory, &stem, action);
                    last = Some(out.clone());
                    Step {
                        program: program.into(),
                        args: vec![image.path.clone().into_os_string(), out.into_os_string()],
                        cwd: directory,
                        make: None,
                    }
                })
                .collect::<Vec<_>>();
            Ok(Plan {
                done: match (images.len(), &last) {
                    (1, Some(out)) => format!("Saved {}", name(out)),
                    (count, _) => format!("Converted {count} images"),
                },
                steps,
                result: last,
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
                steps: vec![Step {
                    program: "wl-copy".into(),
                    args: vec!["--".into(), text.join("\n").into()],
                    cwd: parent,
                    make: None,
                }],
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
                    .map(|file| Step {
                        program: "xdg-open".into(),
                        args: vec![file.path.clone().into_os_string()],
                        cwd: parent.clone(),
                        make: None,
                    })
                    .collect(),
                done: "Opened".into(),
                result: None,
            })
        }
        other => Err(format!("no action {other}")),
    }
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
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

    fn ids(actions: &[Action]) -> Vec<&'static str> {
        actions.iter().map(|action| action.id).collect()
    }

    fn all() -> Vec<String> {
        ALL.iter().map(|action| action.id.to_owned()).collect()
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
        let photo = [file("/nowhere/photo.jpeg", Kind::Image)];
        assert_eq!(
            ids(&offered(&photo, &all(), &everything)),
            ["zip", "png", "webp", "copy", "open"]
        );
        let archive = [file("/nowhere/backup.tar.gz", Kind::Archive)];
        assert!(ids(&offered(&archive, &all(), &everything)).contains(&"extract"));
        // Without the programs, only what needs none of them.
        let nothing = |_: &str| false;
        assert!(offered(&pdfs, &all(), &nothing).is_empty());
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
        assert_eq!(merge.steps[0].program, "pdfunite");
        assert_eq!(
            merge.steps[0].args,
            ["/nowhere/a.pdf", "/nowhere/b.pdf", "/nowhere/Merged.pdf"]
        );
        let merge = plan("merge", &pdfs, &only("qpdf")).unwrap();
        assert_eq!(
            merge.steps[0].args,
            [
                "--empty",
                "--pages",
                "/nowhere/a.pdf",
                "/nowhere/b.pdf",
                "--",
                "/nowhere/Merged.pdf"
            ]
        );

        let zip = plan("zip", &pdfs, &only("zip")).unwrap();
        assert_eq!(
            zip.steps[0].args,
            ["-r", "-q", "/nowhere/Archive.zip", "a.pdf", "b.pdf"]
        );
        assert_eq!(zip.done, "Saved Archive.zip");

        let archive = [file("/nowhere/backup.tar.gz", Kind::Archive)];
        let extract = plan("extract", &archive, &only("tar")).unwrap();
        assert_eq!(
            extract.steps[0].args,
            ["-xf", "/nowhere/backup.tar.gz", "-C", "/nowhere/backup"]
        );
        assert_eq!(
            extract.steps[0].make.as_deref(),
            Some(Path::new("/nowhere/backup"))
        );
        assert_eq!(extract.done, "Extracted to backup");

        let photo = [file("/nowhere/photo.heic", Kind::Image)];
        let convert = plan("jpg", &photo, &only("magick")).unwrap();
        assert_eq!(
            convert.steps[0].args,
            ["/nowhere/photo.heic", "/nowhere/photo.jpg"]
        );
    }
}

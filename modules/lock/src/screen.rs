//! What the lock screen's renderer needs, made ready before any lock: a
//! shell of its own with only Mochi's files, Quickshell, the theme, and the
//! wallpaper blurred.
//!
//! The shell has no module views and no plugin's: the core QML, whose
//! `lock/LockScreen.qml` is the lock screen, and the fonts. Its files are
//! copies, never links into a source tree, and it doesn't reload when they
//! change.

use std::path::{Path, PathBuf};

use mochi_core::assets::{self, Mode};

/// The renderer's `shell.qml`.
const SHELL: &str = r#"import Quickshell
import qs.lock

// Mochi's lock screen, drawn on mochi-lock's private display. Written by
// the lock module; nothing here reloads.
ShellRoot {
    settings.watchFiles: false

    LockScreen {}
}
"#;

/// The modules whose views the lock screen always has: the widgets view
/// that lays it out, and the password field.
pub const ALWAYS: [&str; 2] = ["widgets", "lock"];

/// Writes the renderer's shell into `dir`, with the views of [`ALWAYS`]
/// and of `modules`, the ones whose widgets are on the lock screen, copied
/// from the daemon's `shell`. A module's own views come from `builtin/`
/// when a plugin overrides some of them, never the plugin's; a plugin's
/// widgets are there only when the widgets module let them on.
pub fn write_shell(dir: &Path, shell: Option<&Path>, modules: &[String]) -> std::io::Result<()> {
    // Copies, so a link left from a development build goes too.
    let _ = std::fs::remove_file(dir.join("shell.qml"));
    assets::write_shell(
        dir,
        &mochi_core::QML,
        &[],
        &assets::find_fonts(),
        Mode::Copy,
    )?;
    let _ = std::fs::remove_file(dir.join("shell.qml"));
    std::fs::write(dir.join("shell.qml"), SHELL)?;
    write_views(dir, shell, modules)
}

/// Brings the views in `dir`'s `modules/` in line with the ones the lock
/// screen needs, as [`write_shell`] does, without touching the rest: a
/// view that didn't change is left as it was.
pub fn write_views(dir: &Path, shell: Option<&Path>, modules: &[String]) -> std::io::Result<()> {
    let mut copied = Vec::new();
    if let Some(shell) = shell {
        let wanted = ALWAYS
            .iter()
            .map(|module| (*module).to_owned())
            .chain(modules.iter().cloned());
        for module in wanted {
            // A module id is a name, never a path.
            if module.is_empty() || module.contains(['/', '\\']) || module.starts_with('.') {
                continue;
            }
            if copied.contains(&module) {
                continue;
            }
            let builtin = shell.join("builtin").join(&module);
            let from = if builtin.is_dir() {
                builtin
            } else {
                shell.join("modules").join(&module)
            };
            if from.is_dir() {
                copy_dir(&from, &dir.join("modules").join(&module))?;
                copied.push(module);
            }
        }
    }
    // Quickshell finds the views by the imports here: without them, a view
    // can't use the others in its folder.
    let imports: String = copied
        .iter()
        .enumerate()
        .map(|(index, module)| format!("import \"modules/{module}\" as Module{index}\n"))
        .collect();
    let modules_file = format!(
        "// Written by the lock module: the views the lock screen has.\nimport QtQuick\n{imports}\nQtObject {{}}\n"
    );
    if std::fs::read_to_string(dir.join("Modules.qml"))
        .ok()
        .as_deref()
        != Some(&modules_file)
    {
        std::fs::write(dir.join("Modules.qml"), modules_file)?;
    }
    // The views of modules the lock screen no longer shows.
    if let Ok(entries) = std::fs::read_dir(dir.join("modules")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !copied.contains(&name) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    Ok(())
}

/// Copies a directory's files, following links, so the copy holds the
/// files themselves and nothing that points elsewhere.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    let mut names = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        names.insert(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
            continue;
        }
        // Only what changed: a file left as it was keeps its time.
        let contents = std::fs::read(entry.path())?;
        let unchanged = std::fs::symlink_metadata(&target).is_ok_and(|meta| meta.is_file())
            && std::fs::read(&target).is_ok_and(|old| old == contents);
        if !unchanged {
            let _ = std::fs::remove_file(&target);
            std::fs::write(&target, contents)?;
        }
    }
    // What the module no longer has.
    for entry in std::fs::read_dir(to)?.flatten() {
        if !names.contains(&entry.file_name()) {
            let path = entry.path();
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
        }
    }
    Ok(())
}

/// Quickshell, by its full path, from PATH as mochid has it.
pub fn quickshell() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("quickshell"))
        .find(|candidate| candidate.is_file())
}

/// An image, or the wallpaper with `auto`, blurred into `out`: a
/// small copy, which the renderer stretches smoothly, so a lock never
/// waits on it. `None` when there's no image to use.
pub fn blur(source: Option<String>, out: &Path) -> Option<PathBuf> {
    let wanted = source?;
    let mochi_core::palette::Source::Image(path) = mochi_core::palette::source(&wanted).ok()?
    else {
        return None;
    };
    blurred(image::open(&path).ok()?, out)?;
    Some(out.to_owned())
}

/// Each monitor as it is now, for the lock screen's background: taken
/// before locking, since the lock screen is all there is to see after. A
/// compositor that doesn't answer within a second gives none, so a lock
/// never waits longer. Blurring them comes after, with [`blur_shots`],
/// once the screen is locked.
pub fn capture(dir: &Path) -> Vec<mochi_core::compositor::capture::Frame> {
    let (sent, received) = std::sync::mpsc::channel();
    let scratch = dir.to_owned();
    let started = std::thread::Builder::new()
        .name("lock-shot".into())
        .spawn(move || {
            let _ = sent.send(mochi_core::compositor::capture::capture(&scratch));
        });
    if started.is_err() {
        return Vec::new();
    }
    match received.recv_timeout(std::time::Duration::from_secs(1)) {
        Ok(Ok(frames)) => frames,
        Ok(Err(error)) => {
            tracing::warn!(%error, "can't take the screenshot for the lock screen");
            Vec::new()
        }
        Err(_) => {
            tracing::warn!("the screenshot for the lock screen took too long");
            Vec::new()
        }
    }
}

/// The screenshots, small and blurred, into `dir` as `shot-<monitor>.png`,
/// by monitor.
pub fn blur_shots(
    frames: &[mochi_core::compositor::capture::Frame],
    dir: &Path,
) -> std::collections::BTreeMap<String, PathBuf> {
    frames
        .iter()
        .filter_map(|frame| {
            let (width, height, pixels) = frame.thumbnail(SMALL);
            let image = image::RgbImage::from_raw(width, height, pixels)?;
            let name: String = frame
                .output
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let out = dir.join(format!("shot-{name}.png"));
            blurred(image::DynamicImage::ImageRgb8(image), &out)?;
            Some((frame.output.clone(), out))
        })
        .collect()
}

/// The longer side of a background before it's blurred: the renderer
/// stretches it smoothly, and blurring a small one takes no time.
const SMALL: u32 = 480;

/// A small copy of `image`, blurred, into `out`.
fn blurred(image: image::DynamicImage, out: &Path) -> Option<()> {
    let small = image.resize(SMALL, SMALL, image::imageops::FilterType::Triangle);
    small.blur(6.0).to_rgb8().save(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_shell_of_its_own() {
        let dir = std::env::temp_dir().join(format!("mochi-lock-shell-{}", std::process::id()));
        write_shell(&dir, None, &[]).unwrap();
        let shell = std::fs::read_to_string(dir.join("shell.qml")).unwrap();
        assert!(shell.contains("LockScreen {}"));
        assert!(shell.contains("watchFiles: false"));
        assert!(dir.join("lock/LockScreen.qml").is_file());
        assert!(dir.join("island/Theme.qml").is_file());
        // No module's views, nor a plugin's.
        let modules: Vec<_> = std::fs::read_dir(dir.join("modules")).unwrap().collect();
        assert!(modules.is_empty());
        assert!(
            !std::fs::symlink_metadata(dir.join("shell.qml"))
                .unwrap()
                .is_symlink()
        );
        // Written again, the same.
        write_shell(&dir, None, &[]).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn blurs_an_image_small() {
        let dir = std::env::temp_dir().join(format!("mochi-lock-blur-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("wall.png");
        image::RgbImage::from_fn(1600, 900, |x, _| image::Rgb([(x % 256) as u8, 40, 90]))
            .save(&source)
            .unwrap();
        let out = dir.join("blurred.png");
        assert_eq!(
            blur(Some(source.to_str().unwrap().to_owned()), &out),
            Some(out.clone())
        );
        let made = image::open(&out).unwrap();
        assert!(made.width() <= 480);
        assert_eq!(blur(None, &out), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

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
    std::fs::write(
        dir.join("Modules.qml"),
        format!(
            "// Written by the lock module: the views the lock screen has.\nimport QtQuick\n{imports}\nQtObject {{}}\n"
        ),
    )?;
    Ok(())
}

/// Copies a directory's files, following links, so the copy holds the
/// files themselves and nothing that points elsewhere.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::write(target, std::fs::read(entry.path())?)?;
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

/// Each monitor as it is now, blurred into `dir` as `shot-<monitor>.png`,
/// for the lock screen's background: by monitor. A compositor that doesn't
/// answer within a second leaves it empty, so a lock never waits longer.
pub fn shoot(dir: &Path) -> std::collections::BTreeMap<String, PathBuf> {
    let (sent, received) = std::sync::mpsc::channel();
    let scratch = dir.to_owned();
    let started = std::thread::Builder::new()
        .name("lock-shot".into())
        .spawn(move || {
            let _ = sent.send(mochi_core::compositor::capture::capture(&scratch));
        });
    if started.is_err() {
        return std::collections::BTreeMap::new();
    }
    let frames = match received.recv_timeout(std::time::Duration::from_secs(1)) {
        Ok(Ok(frames)) => frames,
        Ok(Err(error)) => {
            tracing::warn!(%error, "can't take the screenshot for the lock screen");
            return std::collections::BTreeMap::new();
        }
        Err(_) => {
            tracing::warn!("the screenshot for the lock screen took too long");
            return std::collections::BTreeMap::new();
        }
    };
    frames
        .iter()
        .filter_map(|frame| {
            let (width, height) = frame.size();
            let image = image::RgbImage::from_raw(width, height, frame.shown())?;
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

/// A small copy of `image`, blurred, into `out`.
fn blurred(image: image::DynamicImage, out: &Path) -> Option<()> {
    let small = image.resize(480, 480, image::imageops::FilterType::Triangle);
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

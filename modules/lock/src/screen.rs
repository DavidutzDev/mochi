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

/// Writes the renderer's shell into `dir`.
pub fn write_shell(dir: &Path) -> std::io::Result<()> {
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
    std::fs::write(dir.join("shell.qml"), SHELL)
}

/// Quickshell, by its full path, from PATH as mochid has it.
pub fn quickshell() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("quickshell"))
        .find(|candidate| candidate.is_file())
}

/// The wallpaper, or the image `background` names, blurred into `out`: a
/// small copy, which the renderer stretches smoothly, so a lock never
/// waits on it. `None` when there's no image to use.
pub fn blur(background: &str, out: &Path) -> Option<PathBuf> {
    if background == "none" {
        return None;
    }
    let wanted = if background == "wallpaper" {
        "auto"
    } else {
        background
    };
    let mochi_core::palette::Source::Image(path) = mochi_core::palette::source(wanted).ok()? else {
        return None;
    };
    let image = image::open(&path).ok()?;
    let small = image.resize(480, 480, image::imageops::FilterType::Triangle);
    let blurred = small.blur(6.0);
    blurred.to_rgb8().save(out).ok()?;
    Some(out.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_shell_of_its_own() {
        let dir = std::env::temp_dir().join(format!("mochi-lock-shell-{}", std::process::id()));
        write_shell(&dir).unwrap();
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
        write_shell(&dir).unwrap();
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
        assert_eq!(blur(source.to_str().unwrap(), &out), Some(out.clone()));
        let made = image::open(&out).unwrap();
        assert!(made.width() <= 480);
        assert_eq!(blur("none", &out), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

//! Writes the QML tree Quickshell loads.
//!
//! The layout is the core QML at the root and each enabled module's views in
//! `modules/<id>/`. Anything else in the directory is removed. Files whose
//! content didn't change are left alone, because Quickshell reloads whenever
//! a watched file changes.
//!
//! A module with overridden views also gets its own files, untouched, in
//! `builtin/<id>/`. The island loads a view from there when the override
//! fails to load.
//!
//! In [`Mode::Link`] the entries are symlinks into the source tree instead,
//! so editing QML in the repository hot-reloads the running shell.
//!
//! The fonts Mochi's packages bring are linked into `fonts/`, where `Theme`
//! loads them, see [`find_fonts`].

use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use include_dir::{Dir, DirEntry};

use crate::module::Assets;

/// Generated at the root. Never instantiated: Quickshell only watches
/// directories that some QML file imports, and views are loaded by URL at
/// runtime. Importing every module here makes their files hot-reload.
const MODULES_FILE: &str = "Modules.qml";

/// Where the builtin views of overridden modules go, as `builtin/<id>/`.
const BUILTIN_DIR: &str = "builtin";

/// Where the fonts go.
const FONTS_DIR: &str = "fonts";

/// The fonts `Theme` loads: the text font and the icon font.
pub const FONT_FILES: [&str; 2] = ["InterVariable.ttf", "MaterialSymbolsRounded.ttf"];

/// Where packages put the fonts when `MOCHI_FONTS` doesn't say.
const SYSTEM_FONTS: &str = "/usr/share/mochi/fonts";

/// The font files found, by name: in the directories `MOCHI_FONTS` lists
/// (the Nix package sets it), then in `/usr/share/mochi/fonts`. A missing
/// one makes the shell fall back to the system's fonts and drawn symbols.
pub fn find_fonts() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("MOCHI_FONTS")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    dirs.push(PathBuf::from(SYSTEM_FONTS));
    FONT_FILES
        .iter()
        .filter_map(|name| {
            dirs.iter()
                .map(|dir| dir.join(name))
                .find(|path| path.is_file())
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Write the embedded files.
    Copy,
    /// Symlink to the source tree, for hot reload while developing.
    Link,
}

/// One module's views in the shell.
#[derive(Debug, Clone, Copy)]
pub struct ShellModule<'a> {
    pub id: &'a str,
    pub assets: &'a Assets,
    /// Files that replace some of its own, by file name, like
    /// `Compact.qml`: a plugin's override of a builtin view. They are
    /// linked, so they sit in the module's directory and can use its other
    /// files.
    pub overrides: &'a [(String, PathBuf)],
}

/// Returns how many files were written or relinked.
pub fn write_shell(
    out: &Path,
    core: &Assets,
    modules: &[ShellModule<'_>],
    fonts: &[PathBuf],
    mode: Mode,
) -> io::Result<usize> {
    fs::create_dir_all(out.join("modules"))?;
    let mut written = 0;

    let mut keep = vec![
        "modules".to_owned(),
        BUILTIN_DIR.to_owned(),
        FONTS_DIR.to_owned(),
        MODULES_FILE.to_owned(),
    ];
    let (core_dir, core_source) = match core {
        Assets::Embedded { dir, source } => (*dir, *source),
        Assets::Disk(_) => panic!("the core QML is embedded"),
    };
    for entry in core_dir.entries() {
        let name = file_name(entry.path());
        written += match mode {
            Mode::Copy => copy(entry, out)?,
            Mode::Link => link(&Path::new(core_source).join(&name), &out.join(&name))?,
        };
        keep.push(name);
    }
    remove_others(out, &keep)?;

    // Fonts are big and never embedded: always links.
    fs::create_dir_all(out.join(FONTS_DIR))?;
    let mut names = Vec::new();
    for font in fonts {
        let name = file_name(font);
        written += link(font, &out.join(FONTS_DIR).join(&name))?;
        names.push(name);
    }
    remove_others(&out.join(FONTS_DIR), &names)?;

    // A plugin without views, like a launcher provider, has nothing here.
    let modules: Vec<&ShellModule<'_>> = modules
        .iter()
        .filter(|module| match module.assets {
            Assets::Disk(dir) => dir.is_dir() || !module.overrides.is_empty(),
            Assets::Embedded { .. } => true,
        })
        .collect();
    fs::create_dir_all(out.join(BUILTIN_DIR))?;
    let mut builtin = Vec::new();
    for module in &modules {
        let target = out.join("modules").join(module.id);
        if module.overrides.is_empty() {
            written += whole(module.assets, mode, &target)?;
        } else {
            written += overridden(module.assets, mode, module.overrides, &target)?;
            written += whole(module.assets, mode, &out.join(BUILTIN_DIR).join(module.id))?;
            builtin.push(module.id.to_owned());
        }
    }
    let ids: Vec<String> = modules.iter().map(|module| module.id.to_owned()).collect();
    remove_others(&out.join("modules"), &ids)?;
    remove_others(&out.join(BUILTIN_DIR), &builtin)?;

    if write_if_changed(&out.join(MODULES_FILE), modules_file(&ids).as_bytes())? {
        written += 1;
    }
    Ok(written)
}

/// A module's own directory, copied or linked.
fn whole(assets: &Assets, mode: Mode, target: &Path) -> io::Result<usize> {
    match (assets, mode) {
        (Assets::Embedded { dir, .. }, Mode::Copy) => copy_dir(dir, target),
        (Assets::Embedded { source, .. }, Mode::Link) => link(Path::new(source), target),
        (Assets::Disk(dir), _) => link(dir, target),
    }
}

/// A module directory with some files replaced: a real directory where the
/// replacements are links, and the rest is copied or linked one by one.
fn overridden(
    assets: &Assets,
    mode: Mode,
    overrides: &[(String, PathBuf)],
    target: &Path,
) -> io::Result<usize> {
    if is_symlink(target) {
        remove(target)?;
    }
    fs::create_dir_all(target)?;
    let replaced = |name: &str| overrides.iter().any(|(file, _)| file == name);

    let mut written = 0;
    let mut keep: Vec<String> = Vec::new();
    match (assets, mode) {
        (Assets::Embedded { dir, .. }, Mode::Copy) => {
            for child in dir.entries() {
                let name = file_name(child.path());
                if !replaced(&name) {
                    written += copy(child, target)?;
                    keep.push(name);
                }
            }
        }
        (Assets::Embedded { dir, source }, Mode::Link) => {
            for child in dir.entries() {
                let name = file_name(child.path());
                if !replaced(&name) {
                    written += link(&Path::new(source).join(&name), &target.join(&name))?;
                    keep.push(name);
                }
            }
        }
        (Assets::Disk(dir), _) => {
            for child in fs::read_dir(dir)? {
                let name = child?.file_name().to_string_lossy().into_owned();
                if !replaced(&name) {
                    written += link(&dir.join(&name), &target.join(&name))?;
                    keep.push(name);
                }
            }
        }
    }
    for (name, replacement) in overrides {
        written += link(replacement, &target.join(name))?;
        keep.push(name.clone());
    }
    remove_others(target, &keep)?;
    Ok(written)
}

fn modules_file(modules: &[String]) -> String {
    let mut text = String::from(
        "// Generated by mochid. Do not edit.\n\
         // Imports every module so Quickshell watches their files for hot reload.\n\
         import QtQuick\n",
    );
    for (index, module) in modules.iter().enumerate() {
        text.push_str(&format!("import \"modules/{module}\" as Module{index}\n"));
    }
    text.push_str("\nQtObject {}\n");
    text
}

fn link(source: &Path, target: &Path) -> io::Result<usize> {
    if fs::read_link(target).is_ok_and(|current| current == source) {
        return Ok(0);
    }
    remove(target)?;
    symlink(source, target)?;
    Ok(1)
}

/// Copies one embedded entry to `out/<its path>`.
fn copy(entry: &DirEntry, out: &Path) -> io::Result<usize> {
    let target = out.join(entry.path());
    match entry {
        DirEntry::File(file) => Ok(usize::from(write_if_changed(&target, file.contents())?)),
        DirEntry::Dir(dir) => copy_children(dir, out, &target),
    }
}

/// Copies the contents of `dir` into `target`.
fn copy_dir(dir: &Dir, target: &Path) -> io::Result<usize> {
    // Embedded paths are relative to the embedded root, so this works for a
    // root `Dir` too: its children land directly in `target`.
    copy_children(dir, target, target)
}

fn copy_children(dir: &Dir, out: &Path, target: &Path) -> io::Result<usize> {
    if is_symlink(target) {
        remove(target)?;
    }
    fs::create_dir_all(target)?;

    let mut written = 0;
    let mut keep = Vec::new();
    for child in dir.entries() {
        written += copy(child, out)?;
        keep.push(file_name(child.path()));
    }
    remove_others(target, &keep)?;
    Ok(written)
}

/// Writes through a temporary file and a rename, so Quickshell never reads a
/// half-written file. Returns whether anything was written.
fn write_if_changed(path: &Path, contents: &[u8]) -> io::Result<bool> {
    if is_symlink(path) {
        remove(path)?;
    } else if fs::read(path).is_ok_and(|current| current == contents) {
        return Ok(false);
    }

    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);

    fs::write(&temporary, contents)?;
    fs::rename(&temporary, path)?;
    Ok(true)
}

fn remove_others(dir: &Path, keep: &[String]) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !keep.iter().any(|name| entry.file_name() == name.as_str()) {
            remove(&entry.path())?;
        }
    }
    Ok(())
}

/// Removes a file, symlink or directory. Missing paths are fine.
fn remove(path: &Path) -> io::Result<()> {
    let result = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) => Err(error),
    };
    match result {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .expect("embedded paths have a file name")
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    static FIXTURE: Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/module");
    const FIXTURE_ASSETS_SOURCE: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/module");
    static FIXTURE_ASSETS: Assets = Assets::new(&FIXTURE, FIXTURE_ASSETS_SOURCE);

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mochi-assets-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn write(out: &Path, modules: &[&str], mode: Mode) -> usize {
        let modules: Vec<ShellModule<'_>> = modules
            .iter()
            .map(|id| ShellModule {
                id,
                assets: &FIXTURE_ASSETS,
                overrides: &[],
            })
            .collect();
        write_shell(out, &crate::QML, &modules, &[], mode).unwrap()
    }

    #[test]
    fn writes_core_and_modules() {
        let out = scratch("layout");
        write(&out, &["clock"], Mode::Copy);

        assert!(out.join("shell.qml").is_file());
        assert!(out.join("island/Island.qml").is_file());
        assert!(out.join("modules/clock/View.qml").is_file());
        let generated = fs::read_to_string(out.join(MODULES_FILE)).unwrap();
        assert!(generated.contains("import \"modules/clock\" as Module0"));
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn second_write_changes_nothing() {
        let out = scratch("idempotent");
        assert!(write(&out, &["clock"], Mode::Copy) > 0);
        assert_eq!(write(&out, &["clock"], Mode::Copy), 0);
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn disabled_modules_and_stray_files_are_removed() {
        let out = scratch("cleanup");
        write(&out, &["clock", "other"], Mode::Copy);
        fs::write(out.join("stray.qml"), "").unwrap();
        fs::write(out.join("modules/clock/Stray.qml"), "").unwrap();

        write(&out, &["clock"], Mode::Copy);
        assert!(!out.join("modules/other").exists());
        assert!(!out.join("stray.qml").exists());
        assert!(!out.join("modules/clock/Stray.qml").exists());
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn switching_modes_replaces_entries() {
        let out = scratch("modes");
        write(&out, &["clock"], Mode::Link);
        assert!(is_symlink(&out.join("island")));
        assert!(is_symlink(&out.join("modules/clock")));
        assert!(out.join("modules/clock/View.qml").is_file());

        write(&out, &["clock"], Mode::Copy);
        assert!(!is_symlink(&out.join("island")));
        assert!(!is_symlink(&out.join("modules/clock")));
        assert!(out.join("modules/clock/View.qml").is_file());
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn plugins_without_views_are_left_out() {
        let out = scratch("viewless");
        let plugin = Assets::Disk(out.join("nowhere"));
        let modules = [ShellModule {
            id: "plugin",
            assets: &plugin,
            overrides: &[],
        }];
        write_shell(&out, &crate::QML, &modules, &[], Mode::Copy).unwrap();
        assert!(!out.join("modules/plugin").exists());
        let generated = fs::read_to_string(out.join(MODULES_FILE)).unwrap();
        assert!(!generated.contains("modules/plugin"), "{generated}");
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn fonts_are_linked_and_unlinked() {
        let out = scratch("fonts");
        let font = out.with_extension("ttf");
        fs::write(&font, b"font").unwrap();
        write_shell(
            &out,
            &crate::QML,
            &[],
            std::slice::from_ref(&font),
            Mode::Copy,
        )
        .unwrap();
        let linked = out.join("fonts").join(font.file_name().unwrap());
        assert_eq!(fs::read_link(&linked).unwrap(), font);
        write_shell(&out, &crate::QML, &[], &[], Mode::Copy).unwrap();
        assert!(!linked.exists() && out.join("fonts").is_dir());
        fs::remove_dir_all(out).unwrap();
        fs::remove_file(font).unwrap();
    }

    #[test]
    fn plugins_on_disk_are_linked() {
        let out = scratch("disk");
        let plugin = Assets::Disk(PathBuf::from(FIXTURE_ASSETS_SOURCE));
        let modules = [ShellModule {
            id: "plugin",
            assets: &plugin,
            overrides: &[],
        }];
        write_shell(&out, &crate::QML, &modules, &[], Mode::Copy).unwrap();
        assert!(is_symlink(&out.join("modules/plugin")));
        assert!(out.join("modules/plugin/View.qml").is_file());
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn overrides_replace_one_file_in_either_mode() {
        let out = scratch("overrides");
        let replacement = out.with_extension("replacement.qml");
        fs::write(&replacement, "// mine\n").unwrap();
        let overrides = [("View.qml".to_owned(), replacement.clone())];
        for mode in [Mode::Copy, Mode::Link, Mode::Copy] {
            let modules = [ShellModule {
                id: "clock",
                assets: &FIXTURE_ASSETS,
                overrides: &overrides,
            }];
            write_shell(&out, &crate::QML, &modules, &[], mode).unwrap();
            let dir = out.join("modules/clock");
            assert!(!is_symlink(&dir));
            assert_eq!(fs::read_link(dir.join("View.qml")).unwrap(), replacement);
            assert_eq!(
                fs::read_to_string(dir.join("View.qml")).unwrap(),
                "// mine\n"
            );
            // The module's own view stays reachable as a fallback.
            assert_ne!(
                fs::read_to_string(out.join("builtin/clock/View.qml")).unwrap(),
                "// mine\n"
            );
        }
        // Without the override, the module's own file comes back, and the
        // fallback copy goes.
        write(&out, &["clock"], Mode::Copy);
        assert!(!out.join("builtin/clock").exists());
        assert_ne!(
            fs::read_to_string(out.join("modules/clock/View.qml")).unwrap(),
            "// mine\n"
        );
        fs::remove_dir_all(out).unwrap();
        fs::remove_file(replacement).unwrap();
    }
}

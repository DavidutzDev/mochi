//! Saved layouts: arrangements kept under a name, to switch between, in
//! `widget-layouts/` next to `widgets.toml`, one file each, `<name>.toml`,
//! in the same form as `widgets.toml`.
//!
//! The arrangement on the desktop names the saved layout it came from, and
//! each change to it goes to that file too, so switching away and back
//! finds it as it was left. Switching away from an arrangement that has no
//! name and isn't saved keeps it as "Unsaved", so switching never loses
//! widgets.

use std::path::{Path, PathBuf};

use crate::layout::Layout;

pub const DIR: &str = "widget-layouts";
/// Where an arrangement without a name goes when another is put in its
/// place.
pub const UNSAVED: &str = "Unsaved";

/// The longest name, in characters.
const LONGEST: usize = 48;

/// A saved layout, for the drawer.
#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    pub name: String,
    pub layout: Layout,
}

/// A name a layout can be saved under, trimmed, or why not: it becomes a
/// file name.
pub fn check_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a layout needs a name".into());
    }
    if name.chars().count() > LONGEST {
        return Err(format!("a layout's name is at most {LONGEST} characters"));
    }
    if name.starts_with('.')
        || name.contains(['/', '\\', '\0'])
        || name.chars().any(char::is_control)
    {
        return Err(format!(
            "{name:?} can't be a layout's name: it can't start with a dot or have a slash"
        ));
    }
    Ok(name.to_owned())
}

/// The saved layouts in `dir`, by name. Files that don't read are logged
/// and left out.
pub fn list(dir: &Path) -> Vec<Saved> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut saved: Vec<Saved> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "toml") {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_owned();
            check_name(&name).ok()?;
            match Layout::load(&path) {
                Ok(layout) => Some(Saved {
                    name,
                    layout: layout.unwrap_or_default(),
                }),
                Err(error) => {
                    tracing::warn!(%error, "a saved widget layout doesn't read");
                    None
                }
            }
        })
        .collect();
    saved.sort_by_key(|saved| saved.name.to_lowercase());
    saved
}

fn path(dir: &Path, name: &str) -> Result<PathBuf, String> {
    Ok(dir.join(format!("{}.toml", check_name(name)?)))
}

/// Keeps `layout` as `name`, replacing one saved under that name.
pub fn save(dir: &Path, name: &str, layout: &Layout) -> Result<(), String> {
    let path = path(dir, name)?;
    // The file is the name; it doesn't say it again.
    let layout = Layout {
        name: None,
        widgets: layout.widgets.clone(),
    };
    layout
        .save(&path)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// The layout saved as `name`, named after it.
pub fn load(dir: &Path, name: &str) -> Result<Layout, String> {
    let path = path(dir, name)?;
    let mut layout = Layout::load(&path)?.ok_or_else(|| format!("no layout is saved as {name}"))?;
    layout.name = Some(check_name(name)?);
    Ok(layout)
}

pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
    let path = path(dir, name)?;
    std::fs::remove_file(&path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => format!("no layout is saved as {name}"),
        _ => format!("cannot remove {}: {error}", path.display()),
    })
}

/// Whether two arrangements have the same widgets in the same places.
pub fn same(a: &Layout, b: &Layout) -> bool {
    a.widgets == b.widgets
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = r#"
[[widget]]
id = "w1"
module = "widgets"
widget = "clock"
output = "DP-3"
width = 14
height = 7
"#;

    fn dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mochi-saved-{name}-{}", std::process::id()))
    }

    #[test]
    fn names_become_file_names() {
        assert_eq!(check_name("  Work "), Ok("Work".into()));
        assert_eq!(check_name("Late night, 2"), Ok("Late night, 2".into()));
        assert!(check_name("").is_err());
        assert!(check_name("   ").is_err());
        assert!(check_name("../config").is_err());
        assert!(check_name("a/b").is_err());
        assert!(check_name(".hidden").is_err());
        assert!(check_name(&"x".repeat(49)).is_err());
    }

    #[test]
    fn saves_lists_loads_and_deletes() {
        let dir = dir("cycle");
        assert!(list(&dir).is_empty());
        let mut layout = Layout::parse(ONE).unwrap();
        layout.name = Some("Old".into());
        save(&dir, "Work", &layout).unwrap();
        save(&dir, "home", &Layout::default()).unwrap();

        let saved = list(&dir);
        let names: Vec<&str> = saved.iter().map(|saved| saved.name.as_str()).collect();
        assert_eq!(names, ["home", "Work"]);
        assert_eq!(saved[1].layout.widgets.len(), 1);
        // The file doesn't name itself; loading names it.
        assert_eq!(saved[1].layout.name, None);
        let loaded = load(&dir, "Work").unwrap();
        assert_eq!(loaded.name.as_deref(), Some("Work"));
        assert!(same(&loaded, &layout));

        // Saving again replaces it.
        save(&dir, "Work", &Layout::default()).unwrap();
        assert!(load(&dir, "Work").unwrap().widgets.is_empty());

        delete(&dir, "Work").unwrap();
        assert!(delete(&dir, "Work").unwrap_err().contains("no layout"));
        assert!(load(&dir, "Work").unwrap_err().contains("no layout"));
        assert_eq!(list(&dir).len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn leaves_out_files_that_dont_read() {
        let dir = dir("broken");
        save(&dir, "Fine", &Layout::default()).unwrap();
        std::fs::write(dir.join("Broken.toml"), "[[widget]]\nid = 3\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a layout").unwrap();
        let names: Vec<String> = list(&dir).into_iter().map(|saved| saved.name).collect();
        assert_eq!(names, ["Fine"]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

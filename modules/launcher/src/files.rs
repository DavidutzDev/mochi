//! The files provider's index: every file and folder under the roots, by
//! name, in memory, so searching it is as quick as searching apps. Hidden
//! ones and build folders are left out. It's built when the module starts,
//! and again in the background when the launcher opens and it's older than
//! 15 seconds; searches use the old one meanwhile, and the results update
//! when the new one is ready.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::Deserialize;

/// How old the index gets before opening the launcher builds it again.
pub const STALE: Duration = Duration::from_secs(15);

/// `[module.launcher.files]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilesSettings {
    /// Where to look; empty for the home folder.
    pub roots: Vec<String>,
    /// Folder and file names left out, besides hidden ones.
    pub ignore: Vec<String>,
    /// The most entries kept.
    pub max: usize,
}

impl Default for FilesSettings {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            ignore: [
                "node_modules",
                "target",
                "__pycache__",
                "venv",
                "result",
                "dist",
                "build",
            ]
            .map(String::from)
            .to_vec(),
            max: 200_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub folder: bool,
}

#[derive(Debug, Default)]
pub struct Index {
    pub entries: Vec<Entry>,
    pub built: Option<Instant>,
}

impl Index {
    pub fn stale(&self) -> bool {
        self.built.is_none_or(|built| built.elapsed() > STALE)
    }

    /// The best matches for `query` on the name, and on the path for less.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Entry> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut buffer = Vec::new();
        let mut scored: Vec<(u32, &Entry)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let name = entry.path.file_name()?.to_string_lossy();
                let by_name = pattern.score(Utf32Str::new(&name, &mut buffer), &mut matcher);
                let score = match by_name {
                    Some(score) => score * 2,
                    None => {
                        let path = entry.path.to_string_lossy();
                        pattern.score(Utf32Str::new(&path, &mut buffer), &mut matcher)?
                    }
                };
                Some((score, entry))
            })
            .collect();
        // Higher first; then shorter paths, which are usually what's meant.
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.path.as_os_str().len().cmp(&b.1.path.as_os_str().len()))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry)
            .collect()
    }
}

/// The roots, with `~` and an empty list meaning the home folder.
pub fn roots(settings: &FilesSettings) -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if settings.roots.is_empty() {
        return home.into_iter().collect();
    }
    settings
        .roots
        .iter()
        .map(|root| match (root.strip_prefix("~/"), &home) {
            (Some(rest), Some(home)) => home.join(rest),
            _ if root == "~" => home.clone().unwrap_or_default(),
            _ => PathBuf::from(root),
        })
        .collect()
}

/// Walks the roots, breadth first so a limit keeps the shallow entries.
/// Symbolic links aren't followed.
pub fn build(roots: &[PathBuf], settings: &FilesSettings) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut queue: std::collections::VecDeque<PathBuf> = roots.iter().cloned().collect();
    while let Some(dir) = queue.pop_front() {
        let Ok(children) = std::fs::read_dir(&dir) else {
            continue;
        };
        for child in children.flatten() {
            if entries.len() >= settings.max {
                return entries;
            }
            let name = child.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || settings.ignore.iter().any(|ignored| *ignored == *name) {
                continue;
            }
            let Ok(kind) = child.file_type() else {
                continue;
            };
            let path = child.path();
            if kind.is_dir() {
                queue.push_back(path.clone());
            }
            entries.push(Entry {
                path,
                folder: kind.is_dir(),
            });
        }
    }
    entries
}

/// A path with the home folder as `~`, for showing.
pub fn short(path: &Path) -> String {
    let text = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && text.starts_with(&home) => {
            format!("~{}", &text[home.len()..])
        }
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_and_finds_by_name_first() {
        let root = std::env::temp_dir().join(format!("mochi-index-{}", std::process::id()));
        for dir in ["notes/.hidden", "code/target/debug", "code/src", "deep/a/b"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in [
            "notes/report.md",
            "notes/.hidden/report.md",
            "code/target/debug/report",
            "code/src/main.rs",
            "deep/a/b/reporter.txt",
            "reports-old.md",
        ] {
            std::fs::write(root.join(file), "").unwrap();
        }
        let settings = FilesSettings::default();
        let index = Index {
            entries: build(std::slice::from_ref(&root), &settings),
            built: Some(Instant::now()),
        };
        let found: Vec<String> = index
            .search("report", 10)
            .iter()
            .map(|entry| {
                entry
                    .path
                    .strip_prefix(&root)
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect();
        assert!(found.contains(&"notes/report.md".to_owned()), "{found:?}");
        assert!(
            !found
                .iter()
                .any(|path| path.contains(".hidden") || path.contains("target")),
            "{found:?}"
        );
        assert!(index.search("", 10).is_empty());
        assert!(
            index
                .entries
                .iter()
                .any(|entry| entry.folder && entry.path.ends_with("notes"))
        );

        // The limit keeps the shallow ones.
        let small = FilesSettings {
            max: 3,
            ..FilesSettings::default()
        };
        assert!(
            build(std::slice::from_ref(&root), &small)
                .iter()
                .all(|entry| { entry.path.parent() == Some(root.as_path()) })
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

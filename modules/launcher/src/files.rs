//! The files provider's index: every file and folder under the roots, by
//! name, in memory, so searching it is as quick as searching apps. Hidden
//! ones and build folders are left out. A thread builds it when the module
//! starts, then follows changes through inotify: it watches every folder
//! it indexed, and a file or folder made, deleted or moved in one updates
//! the index as it happens. A new folder gets a watch too.
//!
//! Each folder takes one inotify watch. When adding one fails because the
//! system has none left (`fs.inotify.max_user_watches`), the thread logs it
//! once and stops watching; from then on, opening the launcher builds the
//! index again in the background when it's older than 15 seconds, and
//! searches use the old one meanwhile. When inotify drops events because
//! too many came at once, the thread builds the index again from scratch.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::ops::Bound;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;

/// How old the index gets before opening the launcher builds it again,
/// when it isn't watched.
pub const STALE: Duration = Duration::from_secs(15);

/// What a folder's watch reports: its children made, deleted and moved.
/// Symbolic links aren't followed, as when building.
const MASK: WatchMask = WatchMask::CREATE
    .union(WatchMask::DELETE)
    .union(WatchMask::MOVED_FROM)
    .union(WatchMask::MOVED_TO)
    .union(WatchMask::ONLYDIR)
    .union(WatchMask::DONT_FOLLOW)
    .union(WatchMask::EXCL_UNLINK);

/// How often the watching thread checks that the module still runs.
const CHECK: Duration = Duration::from_secs(1);

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

impl FilesSettings {
    /// Whether a name is left out: hidden, or in `ignore`.
    fn skips(&self, name: &OsStr) -> bool {
        let name = name.to_string_lossy();
        name.starts_with('.') || self.ignore.iter().any(|ignored| *ignored == *name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub folder: bool,
}

/// What the indexing thread sends.
#[derive(Debug)]
pub enum Update {
    /// A whole new index. `live` when watches follow it from now on.
    Built { entries: Vec<Entry>, live: bool },
    /// Files and folders made, deleted or moved since, in order.
    Changed(Vec<Change>),
    /// The watches stopped following the folders: the system ran out.
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Added(Entry),
    /// A file, or a folder and everything in it.
    Removed(PathBuf),
}

#[derive(Debug, Default)]
pub struct Index {
    /// Whether each path is a folder. Paths sort by component, so a
    /// folder's contents come right after it.
    entries: BTreeMap<PathBuf, bool>,
    built: Option<Instant>,
    /// Watches keep it up to date.
    live: bool,
    max: usize,
}

impl Index {
    pub fn new(entries: Vec<Entry>, live: bool, max: usize) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|entry| (entry.path, entry.folder))
                .collect(),
            built: Some(Instant::now()),
            live,
            max,
        }
    }

    /// Whether opening the launcher should build it again.
    pub fn stale(&self) -> bool {
        !self.live && self.built.is_none_or(|built| built.elapsed() > STALE)
    }

    /// Stops trusting it to be up to date.
    pub fn lost(&mut self) {
        self.live = false;
    }

    /// Adds and removes entries. Past `max`, new ones are left out.
    pub fn apply(&mut self, changes: Vec<Change>) {
        for change in changes {
            match change {
                Change::Added(entry) => {
                    if self.entries.len() < self.max || self.entries.contains_key(&entry.path) {
                        self.entries.insert(entry.path, entry.folder);
                    }
                }
                Change::Removed(path) => {
                    let inside: Vec<PathBuf> = self
                        .entries
                        .range::<Path, _>((Bound::Excluded(path.as_path()), Bound::Unbounded))
                        .map(|(inside, _)| inside)
                        .take_while(|inside| inside.starts_with(&path))
                        .cloned()
                        .collect();
                    for inside in inside {
                        self.entries.remove(&inside);
                    }
                    self.entries.remove(&path);
                }
            }
        }
    }

    /// The best matches for `query` on the name, and on the path for less.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Entry> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut buffer = Vec::new();
        let mut scored: Vec<(u32, &Path, bool)> = self
            .entries
            .iter()
            .filter_map(|(path, folder)| {
                let name = path.file_name()?.to_string_lossy();
                let by_name = pattern.score(Utf32Str::new(&name, &mut buffer), &mut matcher);
                let score = match by_name {
                    Some(score) => score * 2,
                    None => {
                        let text = path.to_string_lossy();
                        pattern.score(Utf32Str::new(&text, &mut buffer), &mut matcher)?
                    }
                };
                Some((score, path.as_path(), *folder))
            })
            .collect();
        // Higher first; then shorter paths, which are usually what's meant.
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.as_os_str().len().cmp(&b.1.as_os_str().len()))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, path, folder)| Entry {
                path: path.to_owned(),
                folder,
            })
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
    walk(roots, settings, |_| {})
}

/// [`build`], calling `reading` with each folder before reading it.
fn walk(roots: &[PathBuf], settings: &FilesSettings, mut reading: impl FnMut(&Path)) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut queue: std::collections::VecDeque<PathBuf> = roots.iter().cloned().collect();
    while let Some(dir) = queue.pop_front() {
        if entries.len() >= settings.max {
            break;
        }
        reading(&dir);
        let Ok(children) = std::fs::read_dir(&dir) else {
            continue;
        };
        for child in children.flatten() {
            if entries.len() >= settings.max {
                return entries;
            }
            if settings.skips(&child.file_name()) {
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

/// Builds the index on a thread and sends it. With `watch`, the thread
/// stays to follow changes, until the receiver goes away.
pub fn spawn(settings: FilesSettings, watch: bool, sender: UnboundedSender<Update>) {
    let spawned = std::thread::Builder::new()
        .name("mochi-files".into())
        .spawn(move || {
            let roots = roots(&settings);
            if !watch {
                let entries = build(&roots, &settings);
                let _ = sender.send(Update::Built {
                    entries,
                    live: false,
                });
                return;
            }
            // Again from scratch after inotify drops events.
            while follow(&roots, &settings, &sender) {}
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "could not start indexing files");
    }
}

/// Builds the index with a watch on each folder, sends it, and sends what
/// changes. True when it should start over.
fn follow(roots: &[PathBuf], settings: &FilesSettings, sender: &UnboundedSender<Update>) -> bool {
    let mut watcher = match Inotify::init() {
        Ok(inotify) => Watcher {
            inotify,
            folders: HashMap::new(),
            full: false,
        },
        Err(error) => {
            tracing::warn!(%error, "can't watch the indexed folders, so the index is built again when the launcher opens");
            let entries = build(roots, settings);
            let _ = sender.send(Update::Built {
                entries,
                live: false,
            });
            return false;
        }
    };
    let entries = walk(roots, settings, |dir| watcher.watch(dir));
    let live = !watcher.full;
    if !live {
        watcher.warn_full();
    }
    tracing::info!(
        folders = watcher.folders.len(),
        live,
        "watching the indexed folders"
    );
    if sender.send(Update::Built { entries, live }).is_err() || !live {
        return false;
    }

    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let mut poll = libc::pollfd {
            fd: watcher.inotify.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `poll` points at one valid pollfd for the call, and the
        // descriptor stays open while `watcher` lives.
        let ready = unsafe { libc::poll(&raw mut poll, 1, CHECK.as_millis() as libc::c_int) };
        if sender.is_closed() {
            return false;
        }
        if ready <= 0 {
            continue;
        }
        let events = match watcher.inotify.read_events(&mut buffer) {
            Ok(events) => events,
            Err(error) if error.kind() == ErrorKind::WouldBlock => continue,
            Err(error) => {
                tracing::warn!(%error, "could not read the file watches");
                let _ = sender.send(Update::Lost);
                return false;
            }
        };
        let events: Vec<(i32, EventMask, Option<PathBuf>)> = events
            .map(|event| {
                let name = event.name.map(PathBuf::from);
                (event.wd.get_watch_descriptor_id(), event.mask, name)
            })
            .collect();
        let mut changes = Vec::new();
        for (wd, mask, name) in events {
            if mask.contains(EventMask::Q_OVERFLOW) {
                tracing::info!("missed file changes, indexing again");
                return true;
            }
            if mask.contains(EventMask::IGNORED) {
                watcher.folders.remove(&wd);
                continue;
            }
            let (Some((_, folder)), Some(name)) = (watcher.folders.get(&wd), name) else {
                continue;
            };
            if settings.skips(name.as_os_str()) {
                continue;
            }
            let path = folder.join(name);
            let is_dir = mask.contains(EventMask::ISDIR);
            if mask.intersects(EventMask::CREATE | EventMask::MOVED_TO) {
                changes.push(Change::Added(Entry {
                    path: path.clone(),
                    folder: is_dir,
                }));
                if is_dir {
                    // What's in a folder moved here, and what's made in a
                    // new one before its watch is there.
                    let inside = walk(std::slice::from_ref(&path), settings, |dir| {
                        watcher.watch(dir);
                    });
                    changes.extend(inside.into_iter().map(Change::Added));
                }
            } else if mask.intersects(EventMask::DELETE | EventMask::MOVED_FROM) {
                if is_dir {
                    watcher.forget(&path);
                }
                changes.push(Change::Removed(path));
            }
        }
        if watcher.full {
            watcher.warn_full();
            let _ = sender.send(Update::Lost);
            return false;
        }
        if !changes.is_empty() && sender.send(Update::Changed(changes)).is_err() {
            return false;
        }
    }
}

/// The inotify instance and which folder each watch is on, by its number.
#[derive(Debug)]
struct Watcher {
    inotify: Inotify,
    folders: HashMap<i32, (WatchDescriptor, PathBuf)>,
    /// Adding a watch failed for lack of them.
    full: bool,
}

impl Watcher {
    fn watch(&mut self, dir: &Path) {
        if self.full {
            return;
        }
        match self.inotify.watches().add(dir, MASK) {
            Ok(wd) => {
                self.folders
                    .insert(wd.get_watch_descriptor_id(), (wd, dir.to_owned()));
            }
            // ENOSPC: past fs.inotify.max_user_watches.
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::StorageFull | ErrorKind::OutOfMemory
                ) =>
            {
                self.full = true;
            }
            // Gone already, or not ours to read.
            Err(_) => {}
        }
    }

    /// Stops watching a folder moved away or deleted, and the folders in it.
    fn forget(&mut self, dir: &Path) {
        let gone: Vec<i32> = self
            .folders
            .iter()
            .filter(|(_, (_, folder))| folder.starts_with(dir))
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            if let Some((wd, _)) = self.folders.remove(&id) {
                // Its IGNORED event finds nothing to remove then. A deleted
                // folder's watch is gone already, so this fails, which is
                // fine.
                let _ = self.inotify.watches().remove(wd);
            }
        }
    }

    fn warn_full(&self) {
        tracing::warn!(
            watched = self.folders.len(),
            "ran out of inotify watches (fs.inotify.max_user_watches), so the file index is built again when the launcher opens instead"
        );
    }
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
        let index = Index::new(
            build(std::slice::from_ref(&root), &settings),
            false,
            settings.max,
        );
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
        assert_eq!(index.entries.get(&root.join("notes")), Some(&true));

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

    fn entry(path: &str, folder: bool) -> Entry {
        Entry {
            path: PathBuf::from(path),
            folder,
        }
    }

    #[test]
    fn removing_a_folder_removes_what_is_in_it() {
        let mut index = Index::new(
            vec![
                entry("/r/a", true),
                entry("/r/a/x", false),
                entry("/r/a/b", true),
                entry("/r/a/b/y", false),
                entry("/r/a0", false),
                entry("/r/a.txt", false),
            ],
            true,
            10,
        );
        index.apply(vec![Change::Removed("/r/a".into())]);
        let left: Vec<&Path> = index.entries.keys().map(PathBuf::as_path).collect();
        assert_eq!(left, [Path::new("/r/a.txt"), Path::new("/r/a0")]);
    }

    #[test]
    fn additions_stop_at_the_limit() {
        let mut index = Index::new(vec![entry("/r/a", false)], true, 2);
        index.apply(vec![
            Change::Added(entry("/r/b", false)),
            Change::Added(entry("/r/c", false)),
            // Already there: replaced, even when full.
            Change::Added(entry("/r/a", true)),
        ]);
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.entries.get(Path::new("/r/a")), Some(&true));
        assert!(!index.stale());
        index.lost();
        assert!(!index.stale(), "15 seconds pass first");
    }

    /// Waits for the indexing thread's next update.
    fn next(receiver: &mut tokio::sync::mpsc::UnboundedReceiver<Update>) -> Update {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(update) = receiver.try_recv() {
                return update;
            }
            assert!(Instant::now() < deadline, "no update from the watcher");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Applies updates until `done` holds of the index.
    fn until(
        index: &mut Index,
        receiver: &mut tokio::sync::mpsc::UnboundedReceiver<Update>,
        done: impl Fn(&Index) -> bool,
    ) {
        while !done(index) {
            match next(receiver) {
                Update::Changed(changes) => index.apply(changes),
                other => panic!("expected changes, got {other:?}"),
            }
        }
    }

    #[test]
    fn follows_changes_without_building_again() {
        let root = std::env::temp_dir().join(format!("mochi-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes/old.md"), "").unwrap();
        let settings = FilesSettings {
            roots: vec![root.display().to_string()],
            ..FilesSettings::default()
        };
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        spawn(settings.clone(), true, sender);
        let Update::Built { entries, live } = next(&mut receiver) else {
            panic!("the index comes first");
        };
        assert!(live);
        let mut index = Index::new(entries, live, settings.max);
        let built = index.built;
        assert!(index.search("quarterly", 10).is_empty());

        // A new file in a watched folder.
        std::fs::write(root.join("notes/quarterly.md"), "").unwrap();
        until(&mut index, &mut receiver, |index| {
            !index.search("quarterly", 10).is_empty()
        });
        assert_eq!(
            index.search("quarterly", 10)[0].path,
            root.join("notes/quarterly.md")
        );

        // A new folder gets a watch: a file made in it shows up too.
        std::fs::create_dir(root.join("projects")).unwrap();
        std::fs::write(root.join("projects/plan.txt"), "").unwrap();
        until(&mut index, &mut receiver, |index| {
            !index.search("plan.txt", 10).is_empty()
        });
        std::fs::write(root.join("projects/later.txt"), "").unwrap();
        until(&mut index, &mut receiver, |index| {
            !index.search("later.txt", 10).is_empty()
        });

        // Hidden and ignored names stay out.
        std::fs::write(root.join("notes/.secret"), "").unwrap();
        std::fs::create_dir(root.join("projects/target")).unwrap();
        std::fs::write(root.join("notes/marker"), "").unwrap();
        until(&mut index, &mut receiver, |index| {
            !index.search("marker", 10).is_empty()
        });
        assert!(!index.entries.contains_key(&root.join("notes/.secret")));
        assert!(!index.entries.contains_key(&root.join("projects/target")));

        // A rename, and a folder moved out of the way.
        std::fs::rename(root.join("notes/old.md"), root.join("notes/new.md")).unwrap();
        std::fs::rename(root.join("projects"), root.join("notes/archive")).unwrap();
        until(&mut index, &mut receiver, |index| {
            index
                .entries
                .contains_key(&root.join("notes/archive/later.txt"))
        });
        assert!(!index.entries.contains_key(&root.join("notes/old.md")));
        assert!(index.entries.contains_key(&root.join("notes/new.md")));
        assert!(!index.entries.contains_key(&root.join("projects/plan.txt")));
        // The moved folder's watch follows it.
        std::fs::write(root.join("notes/archive/moved.txt"), "").unwrap();
        until(&mut index, &mut receiver, |index| {
            index
                .entries
                .contains_key(&root.join("notes/archive/moved.txt"))
        });

        // A deleted folder takes what's in it.
        std::fs::remove_dir_all(root.join("notes/archive")).unwrap();
        until(&mut index, &mut receiver, |index| {
            !index.entries.contains_key(&root.join("notes/archive"))
        });
        assert!(index.search("plan.txt", 10).is_empty());

        assert_eq!(index.built, built, "never built again");
        drop(receiver);
        std::fs::remove_dir_all(root).unwrap();
    }
}

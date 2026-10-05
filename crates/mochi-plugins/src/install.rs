//! Installing, updating and removing plugins, for `mochi plugins`.
//!
//! This shells out to `git`, `curl` and `tar`, and runs a plugin's build
//! command with `sh`. Nothing runs or lands in place before the caller
//! confirms the [`Plan`]: a `git:` plugin is cloned to a scratch directory
//! first, which runs none of its code, and a release's manifest is read
//! before its asset is downloaded.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::manifest::{self, Manifest, ManifestError};
use crate::{ListError, Locations, Lock, Locked, Source};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct InstallError(pub String);

impl From<ListError> for InstallError {
    fn from(error: ListError) -> Self {
        Self(error.to_string())
    }
}

impl From<ManifestError> for InstallError {
    fn from(error: ManifestError) -> Self {
        Self(error.to_string())
    }
}

impl From<std::io::Error> for InstallError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Install what the lock says, or resolve the source when it has
    /// nothing yet. Installed plugins that match are left alone.
    Install,
    /// Resolve the source again and move the lock.
    Update,
}

/// What installing a plugin will do, for the user to confirm.
#[derive(Debug)]
pub struct Plan {
    pub id: String,
    pub source: Source,
    pub manifest: Manifest,
    /// The commit or tag it resolved to.
    pub revision: Option<String>,
    /// The asset it downloads.
    pub download: Option<String>,
    /// The build command it runs, and where.
    pub build: Option<(String, PathBuf)>,
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let info = &self.manifest.plugin;
        writeln!(f, "{} {} ({})", info.name, info.version, self.id)?;
        if !info.description.is_empty() {
            writeln!(f, "  {}", info.description)?;
        }
        if !info.authors.is_empty() {
            writeln!(f, "  by {}", info.authors.join(", "))?;
        }
        write!(f, "  from    {}", self.source)?;
        match &self.revision {
            Some(revision) => writeln!(f, " at {revision}")?,
            None => writeln!(f)?,
        }
        if let Some(url) = &self.download {
            writeln!(f, "  fetches {url}")?;
        }
        if let Some((command, dir)) = &self.build {
            writeln!(f, "  runs    {command}")?;
            writeln!(f, "          in {}", dir.display())?;
        }
        match &self.manifest.backend {
            Some(backend) => writeln!(f, "  starts  {} with mochid", backend.exec)?,
            None => writeln!(f, "  views only, no backend")?,
        }
        if !self.manifest.uses.state.is_empty() {
            writeln!(
                f,
                "  reads   the state of {}",
                self.manifest.uses.state.join(", ")
            )?;
        }
        if !self.manifest.views.overrides.is_empty() {
            writeln!(f, "  replaces {}", self.manifest.views.overrides.join(", "))?;
        }
        if !self.manifest.actions.is_empty() {
            let names: Vec<&str> = self
                .manifest
                .actions
                .iter()
                .map(|action| action.name.as_str())
                .collect();
            writeln!(f, "  actions {}", names.join(", "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Installed { revision: Option<String> },
    UpToDate { revision: Option<String> },
    Declined,
}

/// Installs plugins, asking `confirm` before each one does anything.
pub struct Installer<'a> {
    pub locations: &'a Locations,
    pub confirm: &'a mut dyn FnMut(&Plan) -> bool,
}

impl fmt::Debug for Installer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Installer")
            .field("locations", self.locations)
            .finish_non_exhaustive()
    }
}

impl Installer<'_> {
    pub fn run(&mut self, id: &str, source: &Source, mode: Mode) -> Result<Outcome, InstallError> {
        let lock = Lock::load(&self.locations.lock)?;
        // The lock only counts for the same source: a changed source in
        // plugins.toml resolves again.
        let locked = lock
            .plugins
            .get(id)
            .filter(|locked| locked.source == source.to_string())
            .cloned();
        let entry = match source {
            Source::Git { url, reference } => {
                self.git(id, source, url, reference.as_deref(), locked, mode)
            }
            Source::GitRelease { owner, repo, tag } => {
                self.release(id, source, owner, repo, tag.as_deref(), locked, mode)
            }
            Source::Path(_) => self.path(id, source, locked, mode),
        };
        // A failure leaves no scratch files behind.
        let work = self.locations.installs.join(format!(".new-{id}"));
        if entry.is_err() {
            let _ = remove_path(&work);
            let _ = remove_path(&work.with_extension("download"));
        }
        let entry = entry?;
        let Some((entry, fresh)) = entry else {
            return Ok(Outcome::Declined);
        };
        let revision = entry.revision();
        let mut lock = Lock::load(&self.locations.lock)?;
        lock.plugins.insert(id.to_owned(), entry);
        lock.save(&self.locations.lock)?;
        Ok(if fresh {
            Outcome::Installed { revision }
        } else {
            Outcome::UpToDate { revision }
        })
    }

    /// Deletes an installed plugin and its lock entry. A `path:` plugin's
    /// directory is the user's and stays. Returns whether there was
    /// anything to remove.
    pub fn remove(&self, id: &str) -> Result<bool, InstallError> {
        manifest::check_id(id).map_err(InstallError)?;
        let mut removed = false;
        let dir = self.locations.installs.join(id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
            removed = true;
        }
        let mut lock = Lock::load(&self.locations.lock)?;
        if lock.plugins.remove(id).is_some() {
            lock.save(&self.locations.lock)?;
            removed = true;
        }
        Ok(removed)
    }

    fn git(
        &mut self,
        id: &str,
        source: &Source,
        url: &str,
        reference: Option<&str>,
        locked: Option<Locked>,
        mode: Mode,
    ) -> Result<Option<(Locked, bool)>, InstallError> {
        let target = self.locations.installs.join(id);
        let work = self.scratch(id)?;
        run(
            "git",
            ["clone", "--quiet", "--filter=blob:none", "--", url]
                .iter()
                .map(OsStr::new)
                .chain([work.as_os_str()]),
            None,
        )?;
        let wanted = match (&locked, mode) {
            (
                Some(Locked {
                    commit: Some(commit),
                    ..
                }),
                Mode::Install,
            ) => Some(commit.as_str()),
            _ => reference,
        };
        let commit = resolve(&work, wanted)?;
        let current = locked.as_ref().and_then(|locked| locked.commit.as_deref());
        if current == Some(commit.as_str()) && target.exists() {
            std::fs::remove_dir_all(&work)?;
            return Ok(Some((locked.expect("matched above"), false)));
        }
        run(
            "git",
            ["-C"]
                .iter()
                .map(OsStr::new)
                .chain([work.as_os_str()])
                .chain(["checkout", "--quiet", "--detach", &commit].map(OsStr::new)),
            None,
        )?;

        let manifest = self.manifest_for(id, &work)?;
        let plan = Plan {
            id: id.to_owned(),
            source: source.clone(),
            revision: Some(commit.chars().take(10).collect()),
            download: None,
            build: build_command(&manifest).map(|command| (command, target.clone())),
            manifest,
        };
        if !(self.confirm)(&plan) {
            std::fs::remove_dir_all(&work)?;
            return Ok(None);
        }
        build(&plan.manifest, &work).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&work);
        })?;
        swap(&work, &target)?;
        Ok(Some((
            Locked {
                source: source.to_string(),
                version: Some(plan.manifest.plugin.version),
                commit: Some(commit),
                ..Locked::default()
            },
            true,
        )))
    }

    #[allow(clippy::too_many_arguments)]
    fn release(
        &mut self,
        id: &str,
        source: &Source,
        owner: &str,
        repo: &str,
        tag: Option<&str>,
        locked: Option<Locked>,
        mode: Mode,
    ) -> Result<Option<(Locked, bool)>, InstallError> {
        let target = self.locations.installs.join(id);
        let pinned = match (&locked, mode) {
            (Some(Locked { tag: Some(tag), .. }), Mode::Install) => Some(tag.clone()),
            _ => tag.map(str::to_owned),
        };
        let tag = match pinned {
            Some(tag) => tag,
            None => latest_release(owner, repo)?,
        };
        if let Some(locked) = &locked
            && locked.tag.as_deref() == Some(tag.as_str())
            && target.exists()
        {
            return Ok(Some((locked.clone(), false)));
        }

        let raw = format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/{tag}/{}",
            manifest::FILE
        );
        let text = fetch(&raw)?;
        let manifest =
            Manifest::parse(&text).map_err(|message| InstallError(format!("{raw}: {message}")))?;
        check_manifest_id(id, &manifest)?;
        let Some(release) = &manifest.release else {
            return Err(InstallError(format!(
                "{id}'s manifest has no [release] section, so it has no release asset: use a git: source to build it"
            )));
        };
        let asset = release
            .asset
            .replace("{id}", id)
            .replace("{version}", &manifest.plugin.version)
            .replace("{tag}", &tag)
            .replace("{arch}", std::env::consts::ARCH);
        let url = format!("https://github.com/{owner}/{repo}/releases/download/{tag}/{asset}");
        let plan = Plan {
            id: id.to_owned(),
            source: source.clone(),
            revision: Some(tag.clone()),
            download: Some(url.clone()),
            build: None,
            manifest,
        };
        if !(self.confirm)(&plan) {
            return Ok(None);
        }

        let work = self.scratch(id)?;
        let archive = work.with_extension("download");
        run(
            "curl",
            ["-fsSL", "--proto", "=https", "-o"]
                .iter()
                .map(OsStr::new)
                .chain([archive.as_os_str(), OsStr::new(&url)]),
            None,
        )?;
        let hash = blake3::hash(&std::fs::read(&archive)?).to_hex().to_string();
        if let Some(Locked {
            tag: Some(locked_tag),
            blake3: Some(locked_hash),
            ..
        }) = &locked
            && *locked_tag == tag
            && *locked_hash != hash
        {
            let _ = std::fs::remove_file(&archive);
            return Err(InstallError(format!(
                "{url} changed since plugins.lock recorded it; run `mochi plugins update {id}` if you trust the new one"
            )));
        }
        std::fs::create_dir_all(&work)?;
        let extracted = run(
            "tar",
            ["-xf"].iter().map(OsStr::new).chain([
                archive.as_os_str(),
                OsStr::new("-C"),
                work.as_os_str(),
            ]),
            None,
        );
        let _ = std::fs::remove_file(&archive);
        extracted?;
        let root = archive_root(&work)?;
        let manifest = self.manifest_for(id, &root)?;
        check_exec(&manifest, &root)?;
        swap(&root, &target)?;
        if root != work {
            let _ = std::fs::remove_dir_all(&work);
        }
        Ok(Some((
            Locked {
                source: source.to_string(),
                version: Some(manifest.plugin.version),
                tag: Some(tag),
                asset: Some(url),
                blake3: Some(hash),
                ..Locked::default()
            },
            true,
        )))
    }

    fn path(
        &mut self,
        id: &str,
        source: &Source,
        locked: Option<Locked>,
        mode: Mode,
    ) -> Result<Option<(Locked, bool)>, InstallError> {
        let dir = self.locations.dir(id, source);
        if mode == Mode::Install
            && let Some(locked) = locked
        {
            return Ok(Some((locked, false)));
        }
        let manifest = self.manifest_for(id, &dir)?;
        let plan = Plan {
            id: id.to_owned(),
            source: source.clone(),
            revision: None,
            download: None,
            build: build_command(&manifest).map(|command| (command, dir.clone())),
            manifest,
        };
        if !(self.confirm)(&plan) {
            return Ok(None);
        }
        build(&plan.manifest, &dir)?;
        Ok(Some((
            Locked {
                source: source.to_string(),
                version: Some(plan.manifest.plugin.version),
                ..Locked::default()
            },
            true,
        )))
    }

    /// An empty scratch directory path for a plugin, next to the installs
    /// so the final rename stays on one file system.
    fn scratch(&self, id: &str) -> Result<PathBuf, InstallError> {
        std::fs::create_dir_all(&self.locations.installs)?;
        let work = self.locations.installs.join(format!(".new-{id}"));
        remove_path(&work)?;
        Ok(work)
    }

    fn manifest_for(&self, id: &str, dir: &Path) -> Result<Manifest, InstallError> {
        let manifest = Manifest::load(dir)?;
        check_manifest_id(id, &manifest)?;
        Ok(manifest)
    }
}

fn check_manifest_id(id: &str, manifest: &Manifest) -> Result<(), InstallError> {
    if manifest.plugin.id != id {
        return Err(InstallError(format!(
            "the plugin calls itself {:?}, plugins.toml calls it {id:?}: use the same id",
            manifest.plugin.id
        )));
    }
    if crate::BUILTIN.contains(&id) {
        return Err(InstallError(format!("{id:?} is a builtin module")));
    }
    Ok(())
}

fn build_command(manifest: &Manifest) -> Option<String> {
    manifest
        .backend
        .as_ref()
        .and_then(|backend| backend.build.clone())
}

/// Runs the build command, if any, on the terminal, then checks the
/// backend is there.
fn build(manifest: &Manifest, dir: &Path) -> Result<(), InstallError> {
    if let Some(command) = build_command(manifest) {
        let status = Command::new("sh")
            .args(["-c", &command])
            .current_dir(dir)
            .env(mochi_protocol::plugin::DIR_ENV, dir)
            .stdin(Stdio::null())
            .status()
            .map_err(|error| InstallError(format!("cannot run sh: {error}")))?;
        if !status.success() {
            return Err(InstallError(format!(
                "the build failed ({status}): {command}"
            )));
        }
    }
    check_exec(manifest, dir)
}

fn check_exec(manifest: &Manifest, dir: &Path) -> Result<(), InstallError> {
    if let Some(backend) = &manifest.backend
        && !dir.join(&backend.exec).is_file()
    {
        return Err(InstallError(format!(
            "the backend {} isn't there after installing",
            dir.join(&backend.exec).display()
        )));
    }
    Ok(())
}

/// The commit `reference` names in a fresh clone: a branch, a tag or a
/// commit. Without one, the default branch.
fn resolve(repo: &Path, reference: Option<&str>) -> Result<String, InstallError> {
    let candidates = match reference {
        Some(reference) => vec![format!("origin/{reference}"), reference.to_owned()],
        None => vec!["HEAD".to_owned()],
    };
    for candidate in &candidates {
        let found = run(
            "git",
            ["-C"].iter().map(OsStr::new).chain([
                repo.as_os_str(),
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new("--quiet"),
                OsStr::new(&format!("{candidate}^{{commit}}")),
            ]),
            None,
        );
        if let Ok(commit) = found {
            return Ok(commit.trim().to_owned());
        }
    }
    let _ = std::fs::remove_dir_all(repo);
    Err(InstallError(format!(
        "the repository has no branch, tag or commit {:?}",
        reference.unwrap_or("HEAD")
    )))
}

fn latest_release(owner: &str, repo: &str) -> Result<String, InstallError> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let text = fetch(&url)?;
    let release: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| InstallError(format!("{url}: {error}")))?;
    release["tag_name"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| InstallError(format!("{url} names no tag")))
}

fn fetch(url: &str) -> Result<String, InstallError> {
    run(
        "curl",
        ["-fsSL", "--proto", "=https", url].map(OsStr::new),
        None,
    )
}

/// Where the manifest is in an extracted archive: at its root, or in its
/// only directory.
fn archive_root(dir: &Path) -> Result<PathBuf, InstallError> {
    if dir.join(manifest::FILE).is_file() {
        return Ok(dir.to_owned());
    }
    let entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    match entries.as_slice() {
        [only] if only.join(manifest::FILE).is_file() => Ok(only.clone()),
        _ => Err(InstallError(format!(
            "the release asset holds no {}",
            manifest::FILE
        ))),
    }
}

/// Puts `new` in place of `target`, which may not exist yet.
fn swap(new: &Path, target: &Path) -> Result<(), InstallError> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if target.exists() {
        let name = target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let old = target.with_file_name(format!(".old-{name}"));
        remove_path(&old)?;
        std::fs::rename(target, &old)?;
        std::fs::rename(new, target)?;
        remove_path(&old)?;
    } else {
        std::fs::rename(new, target)?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Runs a program and returns what it printed, or what went wrong.
fn run<'a>(
    program: &str,
    args: impl IntoIterator<Item = &'a OsStr>,
    dir: Option<&Path>,
) -> Result<String, InstallError> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let output = command
        .output()
        .map_err(|error| InstallError(format!("cannot run {program}: {error}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(InstallError(format!("{program} failed: {message}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mochi-install-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// A plugin repository whose build writes its "backend": a commit on
    /// main, and a later one on a branch.
    fn repository(root: &Path) -> (PathBuf, String, String) {
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "--quiet", "--initial-branch=main"]);
        let manifest = |version: &str| {
            format!(
                "[plugin]\nid = \"timer\"\nname = \"Timer\"\nversion = \"{version}\"\napi = 1\n\n[backend]\nexec = \"bin/timer\"\nbuild = \"mkdir -p bin && printf '#!/bin/sh\\\\n' > bin/timer && chmod +x bin/timer\"\n"
            )
        };
        std::fs::write(repo.join(manifest::FILE), manifest("1.0.0")).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "--quiet", "-m", "one"]);
        let first = git(&repo, &["rev-parse", "HEAD"]);
        git(&repo, &["checkout", "--quiet", "-b", "next"]);
        std::fs::write(repo.join(manifest::FILE), manifest("2.0.0")).unwrap();
        git(&repo, &["commit", "--quiet", "-am", "two"]);
        let second = git(&repo, &["rev-parse", "HEAD"]);
        git(&repo, &["checkout", "--quiet", "main"]);
        (repo, first, second)
    }

    #[test]
    fn git_plugins_install_pin_and_update() {
        let root = scratch("git");
        let (repo, first, second) = repository(&root);
        let locations = Locations {
            list: root.join("plugins.toml"),
            lock: root.join("plugins.lock"),
            installs: root.join("installs"),
        };
        let mut asked = Vec::new();
        let mut confirm = |plan: &Plan| {
            asked.push(plan.to_string());
            true
        };
        {
            let mut installer = Installer {
                locations: &locations,
                confirm: &mut confirm,
            };

            let main: Source = format!("git:file://{}:main", repo.display())
                .parse()
                .unwrap();
            let outcome = installer.run("timer", &main, Mode::Install).unwrap();
            assert_eq!(
                outcome,
                Outcome::Installed {
                    revision: Some(first[..10].to_owned())
                }
            );
            assert!(locations.installs.join("timer/bin/timer").is_file());
            let lock = Lock::load(&locations.lock).unwrap();
            assert_eq!(
                lock.plugins["timer"].commit.as_deref(),
                Some(first.as_str())
            );
            assert_eq!(lock.plugins["timer"].version.as_deref(), Some("1.0.0"));

            // Installing again changes nothing and asks nothing.
            let again = installer.run("timer", &main, Mode::Install).unwrap();
            assert!(matches!(again, Outcome::UpToDate { .. }));

            // Another ref is another source: it resolves anew.
            let next: Source = format!("git:file://{}:next", repo.display())
                .parse()
                .unwrap();
            installer.run("timer", &next, Mode::Install).unwrap();
            let lock = Lock::load(&locations.lock).unwrap();
            assert_eq!(
                lock.plugins["timer"].commit.as_deref(),
                Some(second.as_str())
            );
            assert_eq!(lock.plugins["timer"].version.as_deref(), Some("2.0.0"));

            assert!(installer.remove("timer").unwrap());
            assert!(!locations.installs.join("timer").exists());
            assert!(Lock::load(&locations.lock).unwrap().plugins.is_empty());
        }
        assert_eq!(asked.len(), 2);
        assert!(asked[0].contains("runs    mkdir -p bin"));
        assert!(asked[0].contains("starts  bin/timer"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn declining_leaves_nothing_behind() {
        let root = scratch("decline");
        let (repo, _, _) = repository(&root);
        let locations = Locations {
            list: root.join("plugins.toml"),
            lock: root.join("plugins.lock"),
            installs: root.join("installs"),
        };
        let mut confirm = |_: &Plan| false;
        let mut installer = Installer {
            locations: &locations,
            confirm: &mut confirm,
        };
        let source: Source = format!("git:file://{}", repo.display()).parse().unwrap();
        assert_eq!(
            installer.run("timer", &source, Mode::Install).unwrap(),
            Outcome::Declined
        );
        assert_eq!(std::fs::read_dir(&locations.installs).unwrap().count(), 0);
        assert!(!locations.lock.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_wrong_id_or_ref_is_explained() {
        let root = scratch("wrong");
        let (repo, _, _) = repository(&root);
        let locations = Locations {
            list: root.join("plugins.toml"),
            lock: root.join("plugins.lock"),
            installs: root.join("installs"),
        };
        let mut confirm = |_: &Plan| true;
        let mut installer = Installer {
            locations: &locations,
            confirm: &mut confirm,
        };
        let source: Source = format!("git:file://{}", repo.display()).parse().unwrap();
        let error = installer.run("other", &source, Mode::Install).unwrap_err();
        assert!(error.0.contains("calls itself \"timer\""), "{error}");
        let missing: Source = format!("git:file://{}:nope", repo.display())
            .parse()
            .unwrap();
        let error = installer.run("timer", &missing, Mode::Install).unwrap_err();
        assert!(
            error.0.contains("no branch, tag or commit \"nope\""),
            "{error}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

//! Installing, updating and removing plugins, for `mochi plugins`.
//!
//! This shells out to `git`, `curl` and `tar`, and runs a plugin's build
//! command with `sh`, or `nix build` when the plugin has a `flake.nix` and
//! Nix is installed. Nothing runs or lands in place before the caller
//! confirms the [`Plan`]: a `git:` plugin is cloned to a scratch directory
//! first, which runs none of its code, and a release's manifest is read
//! before its asset is downloaded.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::forge::Releases;
use crate::manifest::{self, Manifest, ManifestError};
use crate::{ListError, Locations, Lock, Locked, Source, registry};

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
    /// How it builds, and where.
    pub build: Option<(Build, PathBuf)>,
}

/// How a plugin's backend is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Build {
    /// The manifest's `build` command, run with `sh`.
    Command(String),
    /// `nix build` on the plugin's `flake.nix`, whose default package has
    /// the backend at the manifest's `exec`, or in `bin/` by its name.
    Flake,
    /// `nix build` with Mochi's own builder, `lib.buildPlugin`, from the
    /// plugin's lock file: for when the tools to build it, or the programs
    /// it needs, aren't installed but Nix is.
    Nix,
}

impl fmt::Display for Build {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(command) => f.write_str(command),
            Self::Flake => f.write_str("nix build .#default, from its flake.nix"),
            Self::Nix => f.write_str("nix build, with Mochi's plugin builder"),
        }
    }
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
        if let Some((build, dir)) = &self.build {
            writeln!(f, "  runs    {build}")?;
            writeln!(f, "          in {}", dir.display())?;
        }
        match &self.manifest.backend {
            Some(backend) => {
                writeln!(f, "  starts  {} with mochid", backend.exec)?;
                if !backend.needs.is_empty() {
                    let missing = self.manifest.missing_needs();
                    write!(f, "  needs   {}", backend.needs.join(", "))?;
                    if missing.is_empty()
                        || matches!(self.build, Some((Build::Nix | Build::Flake, _)))
                    {
                        writeln!(f)?;
                    } else {
                        writeln!(f, " (not installed: {})", missing.join(", "))?;
                    }
                }
            }
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

/// How installing reaches the network: [`Curl`], or files in tests.
pub trait Fetch {
    /// What `url` holds, as text.
    fn text(&self, url: &str) -> Result<String, InstallError>;
    /// Downloads `url` to the file `to`.
    fn download(&self, url: &str, to: &Path) -> Result<(), InstallError>;
}

/// Fetches with `curl`, over HTTPS only, redirects included.
#[derive(Debug, Clone, Copy, Default)]
pub struct Curl;

const CURL: [&str; 5] = ["-fsSL", "--proto", "=https", "--proto-redir", "=https"];

impl Fetch for Curl {
    fn text(&self, url: &str) -> Result<String, InstallError> {
        run("curl", CURL.iter().chain([&url]).map(OsStr::new), None)
            .map_err(|error| naming(url, error))
    }

    fn download(&self, url: &str, to: &Path) -> Result<(), InstallError> {
        run(
            "curl",
            CURL.iter()
                .map(OsStr::new)
                .chain([OsStr::new("-o"), to.as_os_str(), OsStr::new(url)]),
            None,
        )
        .map(drop)
        .map_err(|error| naming(url, error))
    }
}

/// `error` saying which URL it was about.
fn naming(url: &str, error: InstallError) -> InstallError {
    InstallError(format!("{url}: {}", error.0))
}

/// Installs plugins, asking `confirm` before each one does anything.
pub struct Installer<'a> {
    pub locations: &'a Locations,
    pub confirm: &'a mut dyn FnMut(&Plan) -> bool,
    pub fetch: &'a dyn Fetch,
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
            Source::GitRelease { host, repo, tag } => {
                self.release(id, source, host, repo, tag.as_deref(), locked, mode)
            }
            Source::Path(_) => self.path(id, source, locked, mode),
            Source::Registry {
                registry,
                id: package,
                version,
            } => self.registry(
                id,
                source,
                registry.as_deref(),
                package,
                version.as_deref(),
                locked,
                mode,
            ),
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
        remove_path(&self.locations.nix_root(id))?;
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
            build: how_to_build(&manifest, &work).map(|build| (build, target.clone())),
            manifest,
        };
        if !(self.confirm)(&plan) {
            std::fs::remove_dir_all(&work)?;
            return Ok(None);
        }
        self.build(&plan, &work).inspect_err(|_| {
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
        host: &str,
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
        // What's installed already, when it's at `tag`.
        let current = |tag: &str| {
            locked
                .as_ref()
                .filter(|locked| locked.tag.as_deref() == Some(tag) && target.exists())
                .map(|locked| (locked.clone(), false))
        };
        if let Some(current) = pinned.as_deref().and_then(current) {
            return Ok(Some(current));
        }
        let releases = Releases::find(host, repo, self.fetch)?;
        let tag = match pinned {
            Some(tag) => tag,
            None => releases.latest(self.fetch)?,
        };
        if let Some(current) = current(&tag) {
            return Ok(Some(current));
        }

        let raw = releases.manifest_url(&tag);
        let text = self.fetch.text(&raw)?;
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
        let url = releases.asset_url(&tag, &asset, self.fetch)?;
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
        self.fetch.download(&url, &archive)?;
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
        // Nix built it already, like home-manager's `plugins.<id>.src`, and
        // the store can't be written to.
        let built = dir.starts_with("/nix/store");
        let plan = Plan {
            id: id.to_owned(),
            source: source.clone(),
            revision: None,
            download: None,
            build: how_to_build(&manifest, &dir)
                .filter(|_| !built)
                .map(|build| (build, dir.clone())),
            manifest,
        };
        if !(self.confirm)(&plan) {
            return Ok(None);
        }
        self.build(&plan, &dir)?;
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
    /// A plugin from a Bento registry: the release the lock pins, or the
    /// one asked for, or the newest this Mochi runs, cloned at the commit
    /// the registry lists. One it withdrew as malicious is refused.
    #[allow(clippy::too_many_arguments)]
    fn registry(
        &mut self,
        id: &str,
        source: &Source,
        name: Option<&str>,
        package_id: &str,
        version: Option<&str>,
        locked: Option<Locked>,
        mode: Mode,
    ) -> Result<Option<(Locked, bool)>, InstallError> {
        let registry = registry::registry(self.locations, name).map_err(InstallError)?;
        let index = registry::load(&registry, &registry::cache_dir(), mode == Mode::Update)
            .map_err(InstallError)?;
        let package = index.get(package_id).map_err(InstallError)?;
        if package.kind != registry::Kind::Plugin {
            return Err(InstallError(format!(
                "{package_id} is a {} in the registry, not a plugin: `mochi bento add {package_id}` installs it",
                package.kind.as_str()
            )));
        }
        let pinned = locked
            .as_ref()
            .and_then(|locked| locked.commit.clone())
            .filter(|_| mode == Mode::Install);
        let commit = match pinned {
            Some(commit) => commit,
            None => package.pick(version).map_err(InstallError)?.commit.clone(),
        };
        if let Some(release) = package.at(&commit)
            && let Some(why) = &release.malicious
        {
            return Err(InstallError(format!(
                "the registry withdrew {package_id} {} as harmful: {why}",
                release.version
            )));
        }
        let url = package.repository.clone();
        self.git(id, source, &url, Some(&commit), locked, mode)
    }

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

/// How the plugin in `dir` builds, when it has a backend: with its flake
/// when it has one and Nix is installed; with Mochi's Nix builder when Nix
/// is installed but the build's tools or the programs the plugin needs
/// aren't, or there's no backend and nothing to build it with; else with
/// its build command.
/// How a plugin in `dir` would be built, for showing before it is.
pub fn how_to_build(manifest: &Manifest, dir: &Path) -> Option<Build> {
    let backend = manifest.backend.as_ref()?;
    let nix = crate::on_path("nix");
    if dir.join("flake.nix").is_file() && nix {
        return Some(Build::Flake);
    }
    let lacking = backend
        .build
        .as_deref()
        .is_some_and(|command| !missing_tools(command).is_empty())
        || !manifest.missing_needs().is_empty()
        || (backend.build.is_none() && !dir.join(&backend.exec).exists());
    if nix && lacking {
        return Some(Build::Nix);
    }
    backend.build.clone().map(Build::Command)
}

/// The Mochi flake whose `lib.buildPlugin` builds plugins: this version's
/// tag, or `MOCHI_FLAKE`.
fn mochi_flake() -> String {
    std::env::var("MOCHI_FLAKE")
        .ok()
        .filter(|flake| !flake.is_empty())
        .unwrap_or_else(|| format!("github:DavidutzDev/mochi/v{}", env!("CARGO_PKG_VERSION")))
}

/// The Nix expression building the plugin in `dir` with Mochi's builder,
/// against the nixpkgs Mochi's flake pins.
fn builder_expression(dir: &Path) -> String {
    let quote = |text: &str| serde_json::to_string(text).expect("strings serialize");
    format!(
        r#"let
  mochi = builtins.getFlake {flake};
  pkgs = mochi.inputs.nixpkgs.legacyPackages.${{builtins.currentSystem}};
  src = builtins.path {{
    path = {dir};
    name = "source";
    filter = path: _: baseNameOf path != ".git";
  }};
in
mochi.lib.buildPlugin pkgs {{ inherit src; }}"#,
        flake = quote(&mochi_flake()),
        dir = quote(&dir.display().to_string()),
    )
}

impl Installer<'_> {
    /// Builds the plugin in `dir` as the plan says, on the terminal, then
    /// checks the backend is there.
    fn build(&self, plan: &Plan, dir: &Path) -> Result<(), InstallError> {
        match plan.build.as_ref().map(|(build, _)| build) {
            Some(Build::Command(command)) => {
                let status = Command::new("sh")
                    .args(["-c", command])
                    .current_dir(dir)
                    .env(mochi_protocol::plugin::DIR_ENV, dir)
                    .stdin(Stdio::null())
                    .status()
                    .map_err(|error| InstallError(format!("cannot run sh: {error}")))?;
                if !status.success() {
                    return Err(InstallError(format!(
                        "the build failed ({status}): {command}{}",
                        advice(plan, command)
                    )));
                }
            }
            Some(Build::Flake) => {
                let flake = format!("{}#default", dir.display());
                self.build_with_nix(plan, dir, &[flake])?
            }
            Some(Build::Nix) => self.build_with_nix(
                plan,
                dir,
                &["--impure".into(), "--expr".into(), builder_expression(dir)],
            )?,
            None => {}
        }
        check_exec(&plan.manifest, dir)
    }

    /// `nix build` with `what` to build. The result stays alive through a
    /// garbage collector root in the installs' `.nix` directory, and the
    /// backend in the plugin's directory links into it.
    fn build_with_nix(&self, plan: &Plan, dir: &Path, what: &[String]) -> Result<(), InstallError> {
        let exec = plan
            .manifest
            .backend
            .as_ref()
            .map(|backend| backend.exec.clone())
            .unwrap_or_default();
        let root = self.locations.nix_root(&plan.id);
        if let Some(parent) = root.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let status = Command::new("nix")
            .args([
                "--extra-experimental-features",
                "nix-command flakes",
                "build",
                "--out-link",
            ])
            .arg(&root)
            .args(what)
            .stdin(Stdio::null())
            .status()
            .map_err(|error| InstallError(format!("cannot run nix: {error}")))?;
        if !status.success() {
            return Err(InstallError(format!(
                "nix build failed ({status}); its output is above"
            )));
        }
        let output = std::fs::canonicalize(&root)?;
        let name = Path::new(&exec).file_name().unwrap_or_default();
        let built = [output.join(&exec), output.join("bin").join(name)]
            .into_iter()
            .find(|path| path.is_file())
            .ok_or_else(|| {
                InstallError(format!(
                    "the built package {} has neither {exec} nor bin/{}",
                    output.display(),
                    name.to_string_lossy()
                ))
            })?;
        let link = dir.join(&exec);
        if let Some(parent) = link.parent() {
            std::fs::create_dir_all(parent)?;
        }
        remove_path(&link)?;
        std::os::unix::fs::symlink(&built, &link)?;
        Ok(())
    }
}

/// What to try when a build command failed: the tools it lacks, and the
/// ways around building it here.
fn advice(plan: &Plan, command: &str) -> String {
    let missing = missing_tools(command);
    let mut text = String::new();
    if missing.is_empty() {
        text.push_str("\n  Its output is above.");
    } else {
        let names: Vec<String> = missing.iter().map(|tool| format!("`{tool}`")).collect();
        text.push_str(&format!(
            "\n  {} isn't installed. Ways around it:",
            names.join(", ")
        ));
        text.push_str(&format!(
            "\n  - install it, then run `mochi plugins install {}` again",
            plan.id
        ));
    }
    let id = &plan.id;
    text.push_str(&format!(
        "\n  - with home-manager, let Nix build it during the switch, with no tools here:\n      programs.mochi.plugins.{id}.src = <a flake input of its repository>;"
    ));
    if plan.manifest.release.is_some()
        && let Source::Git { url, .. } = &plan.source
        && let Some(release) = release_source(url)
    {
        text.push_str(&format!(
            "\n  - use its prebuilt releases, if it publishes them:\n      {id} = {{ source = \"{release}\" }}"
        ));
    }
    if !crate::on_path("nix") {
        text.push_str(
            "\n  - install Nix: with it, Mochi builds plugins from their lock files, with no other tools",
        );
    }
    text
}

/// The commands `command` starts that aren't on the PATH: the first word
/// of each part between `&&`, `||`, `;` and `|`, past variable settings.
fn missing_tools(command: &str) -> Vec<String> {
    let mut missing: Vec<String> = Vec::new();
    for part in command.split(['&', '|', ';', '\n']) {
        let Some(program) = part
            .split_whitespace()
            .find(|word| !word.contains('=') || word.starts_with(['/', '.']))
        else {
            continue;
        };
        let builtin = matches!(
            program,
            "cd" | "export" | "set" | "test" | "[" | "true" | "false" | "echo"
        );
        if !builtin && !crate::on_path(program) && !missing.iter().any(|tool| tool == program) {
            missing.push(program.to_owned());
        }
    }
    missing
}

/// The `git-release:` source for the repository at a clone URL, when it
/// can be one.
fn release_source(url: &str) -> Option<Source> {
    let location = match url.strip_prefix("https://") {
        Some(rest) => rest.to_owned(),
        None => url.strip_prefix("git@")?.replacen(':', "/", 1),
    };
    format!("git-release:{}", location.trim_end_matches('/'))
        .parse()
        .ok()
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
/// Clones `url` into `into`, which mustn't exist, at `reference`, a
/// branch, tag or commit, or the default branch without one. Runs none of
/// the repository's code. Returns the commit.
pub fn clone(url: &str, reference: Option<&str>, into: &Path) -> Result<String, InstallError> {
    run(
        "git",
        ["clone", "--quiet", "--filter=blob:none", "--", url]
            .iter()
            .map(OsStr::new)
            .chain([into.as_os_str()]),
        None,
    )?;
    let commit = resolve(into, reference)?;
    run(
        "git",
        ["-C"]
            .iter()
            .map(OsStr::new)
            .chain([into.as_os_str()])
            .chain(["checkout", "--quiet", "--detach", &commit].map(OsStr::new)),
        None,
    )?;
    Ok(commit)
}

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
pub(crate) mod tests {
    use super::*;

    /// Answers for URLs, in place of the network. Any other URL fails like
    /// a 404.
    #[derive(Debug, Default)]
    pub(crate) struct Fixtures(std::collections::HashMap<String, Vec<u8>>);

    impl Fixtures {
        pub(crate) fn with(mut self, url: &str, body: impl Into<Vec<u8>>) -> Self {
            self.0.insert(url.to_owned(), body.into());
            self
        }

        fn get(&self, url: &str) -> Result<&[u8], InstallError> {
            self.0
                .get(url)
                .map(Vec::as_slice)
                .ok_or_else(|| InstallError(format!("curl failed: (22) {url}: error 404")))
        }
    }

    impl Fetch for Fixtures {
        fn text(&self, url: &str) -> Result<String, InstallError> {
            Ok(String::from_utf8_lossy(self.get(url)?).into_owned())
        }

        fn download(&self, url: &str, to: &Path) -> Result<(), InstallError> {
            Ok(std::fs::write(to, self.get(url)?)?)
        }
    }

    /// A tar.gz of `files` in a directory `top`, as a release packs a
    /// plugin.
    pub(crate) fn tarball(root: &Path, top: &str, files: &[(&str, &str)]) -> Vec<u8> {
        let pack = root.join("pack");
        let _ = std::fs::remove_dir_all(&pack);
        for (path, text) in files {
            let path = pack.join(top).join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
        }
        let archive = root.join("pack.tar.gz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&pack)
            .arg(top)
            .status()
            .unwrap();
        assert!(status.success());
        let bytes = std::fs::read(&archive).unwrap();
        std::fs::remove_dir_all(&pack).unwrap();
        std::fs::remove_file(&archive).unwrap();
        bytes
    }

    fn locations(root: &Path) -> Locations {
        Locations {
            list: root.join("plugins.toml"),
            lock: root.join("plugins.lock"),
            bento: root.join("bento.toml"),
            installs: root.join("installs"),
        }
    }

    const RELEASED: &str = "[plugin]\nid = \"clock\"\nname = \"Clock\"\nversion = \"1.0.0\"\napi = 1\n[backend]\nexec = \"bin/clock\"\nbuild = \"mochi-no-such-cargo build\"\n[release]\nasset = \"clock-{version}-{arch}.tar.gz\"\n";

    #[test]
    fn installs_a_release_from_a_forgejo() {
        let root = scratch("forgejo");
        let locations = locations(&root);
        let asset = format!("clock-1.0.0-{}.tar.gz", std::env::consts::ARCH);
        let download = format!("https://codeberg.org/User/clock/releases/download/v1.0.0/{asset}");
        let release = |archive: Vec<u8>| {
            Fixtures::default()
                .with(
                    "https://codeberg.org/api/v1/repos/User/clock/releases/latest",
                    r#"{"tag_name": "v1.0.0"}"#,
                )
                .with(
                    "https://codeberg.org/api/v1/repos/User/clock/raw/mochi-plugin.toml?ref=v1.0.0",
                    RELEASED,
                )
                .with(
                    "https://codeberg.org/api/v1/repos/User/clock/releases/tags/v1.0.0",
                    format!(
                        r#"{{"assets": [{{"name": "{asset}", "browser_download_url": "{download}"}}]}}"#
                    ),
                )
                .with(&download, archive)
        };
        let built = [
            ("mochi-plugin.toml", RELEASED),
            ("bin/clock", "#!/bin/sh\n"),
        ];
        let fetch = release(tarball(&root, "clock", &built));
        let source: Source = "git-release:codeberg.org/User/clock".parse().unwrap();
        let mut asked = Vec::new();
        let mut confirm = |plan: &Plan| {
            asked.push(plan.to_string());
            true
        };
        let mut installer = Installer {
            locations: &locations,
            confirm: &mut confirm,
            fetch: &fetch,
        };
        assert_eq!(
            installer.run("clock", &source, Mode::Install).unwrap(),
            Outcome::Installed {
                revision: Some("v1.0.0".into())
            }
        );
        assert!(locations.installs.join("clock/bin/clock").is_file());
        let locked = Lock::load(&locations.lock).unwrap().plugins["clock"].clone();
        assert_eq!(locked.tag.as_deref(), Some("v1.0.0"));
        assert_eq!(locked.asset.as_deref(), Some(download.as_str()));
        assert_eq!(locked.source, "git-release:codeberg.org/User/clock");

        // Installed and locked: nothing to fetch.
        let offline = Fixtures::default();
        installer.fetch = &offline;
        assert!(matches!(
            installer.run("clock", &source, Mode::Install).unwrap(),
            Outcome::UpToDate { .. }
        ));

        // The same release with other bytes is refused.
        std::fs::remove_dir_all(locations.installs.join("clock")).unwrap();
        let changed = release(tarball(
            &root,
            "clock",
            &[built[0], ("bin/clock", "#!/bin/sh\nexit 1\n")],
        ));
        installer.fetch = &changed;
        let error = installer
            .run("clock", &source, Mode::Install)
            .unwrap_err()
            .0;
        assert!(
            error.contains("changed since plugins.lock recorded it"),
            "{error}"
        );
        assert!(
            asked[0].contains(&format!("fetches {download}")),
            "{}",
            asked[0]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

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
            bento: root.join("bento.toml"),
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
                fetch: &Fixtures::default(),
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
            bento: root.join("bento.toml"),
            installs: root.join("installs"),
        };
        let mut confirm = |_: &Plan| false;
        let mut installer = Installer {
            locations: &locations,
            confirm: &mut confirm,
            fetch: &Fixtures::default(),
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
    fn finds_the_tools_a_build_lacks() {
        assert_eq!(
            missing_tools("cargo build --release && install -Dm755 target/release/x bin/x")
                .into_iter()
                .filter(|tool| tool == "cargo")
                .count(),
            usize::from(!crate::on_path("cargo"))
        );
        assert_eq!(
            missing_tools(
                "CC=clang mochi-no-such-tool-1 x | mochi-no-such-tool-2; cd x && mochi-no-such-tool-1"
            ),
            ["mochi-no-such-tool-1", "mochi-no-such-tool-2"]
        );
        assert!(missing_tools("sh -c true && echo done").is_empty());
        assert!(
            missing_tools("/nonexistent/bin/tool").contains(&"/nonexistent/bin/tool".to_owned())
        );
    }

    #[test]
    fn turns_clone_urls_into_release_sources() {
        let release = |url: &str| release_source(url).map(|source| source.to_string());
        assert_eq!(
            release("https://github.com/Xonex5/mochi-clock").unwrap(),
            "git-release:github.com/Xonex5/mochi-clock"
        );
        assert_eq!(
            release("https://github.com/a/b.git/").unwrap(),
            "git-release:github.com/a/b"
        );
        assert_eq!(
            release("git@github.com:a/b.git").unwrap(),
            "git-release:github.com/a/b"
        );
        assert_eq!(
            release("https://codeberg.org/a/b").unwrap(),
            "git-release:codeberg.org/a/b"
        );
        assert!(release("https://github.com/a").is_none());
        assert!(release("file:///tmp/repo").is_none());
    }

    #[test]
    fn a_failed_build_says_what_to_try() {
        let root = scratch("advice");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(manifest::FILE),
            "[plugin]\nid = \"clock\"\nname = \"Clock\"\nversion = \"1\"\napi = 1\n[backend]\nexec = \"bin/clock\"\nbuild = \"mochi-no-such-cargo build\"\n[release]\nasset = \"clock-{version}.tar.gz\"\n",
        )
        .unwrap();
        let manifest = Manifest::load(&root).unwrap();
        let plan = Plan {
            id: "clock".into(),
            source: "git:github.com/Someone/mochi-clock:main".parse().unwrap(),
            manifest,
            revision: None,
            download: None,
            build: Some((
                Build::Command("mochi-no-such-cargo build".into()),
                root.clone(),
            )),
        };
        let locations = Locations {
            list: root.join("plugins.toml"),
            lock: root.join("plugins.lock"),
            bento: root.join("bento.toml"),
            installs: root.join("installs"),
        };
        let installer = Installer {
            locations: &locations,
            confirm: &mut |_| true,
            fetch: &Fixtures::default(),
        };
        let error = installer.build(&plan, &root).unwrap_err().0;
        assert!(
            error.contains("`mochi-no-such-cargo` isn't installed"),
            "{error}"
        );
        assert!(
            error.contains("programs.mochi.plugins.clock.src"),
            "{error}"
        );
        assert!(
            error.contains("git-release:github.com/Someone/mochi-clock"),
            "{error}"
        );
        assert_eq!(
            error.contains("install Nix"),
            !crate::on_path("nix"),
            "{error}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn builds_with_the_flake_only_when_there_is_one() {
        let manifest = Manifest::parse(
            "[plugin]\nid = \"x\"\nname = \"X\"\nversion = \"1\"\napi = 1\n[backend]\nexec = \"bin/x\"\nbuild = \"cargo build\"\n",
        )
        .unwrap();
        assert_eq!(
            how_to_build(&manifest, Path::new("/nonexistent")),
            Some(Build::Command("cargo build".into()))
        );
        let root = scratch("flake");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("flake.nix"), "{}").unwrap();
        let expected = if crate::on_path("nix") {
            Build::Flake
        } else {
            Build::Command("cargo build".into())
        };
        assert_eq!(how_to_build(&manifest, &root), Some(expected));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn builds_with_nix_when_the_tools_are_missing() {
        let manifest = Manifest::parse(
            "[plugin]\nid = \"x\"\nname = \"X\"\nversion = \"1\"\napi = 1\n[backend]\nexec = \"x.py\"\nbuild = \"mochi-no-such-tool build\"\n",
        )
        .unwrap();
        let expected = if crate::on_path("nix") {
            Build::Nix
        } else {
            Build::Command("mochi-no-such-tool build".into())
        };
        assert_eq!(
            how_to_build(&manifest, Path::new("/nonexistent")),
            Some(expected)
        );

        let needs = Manifest::parse(
            "[plugin]\nid = \"x\"\nname = \"X\"\nversion = \"1\"\napi = 1\n[backend]\nexec = \"x.py\"\nneeds = [\"sh\", \"mochi-no-such-tool\"]\nkind = \"python\"\n",
        )
        .unwrap();
        assert_eq!(needs.missing_needs(), ["mochi-no-such-tool"]);
        assert_eq!(
            how_to_build(&needs, Path::new("/nonexistent")),
            crate::on_path("nix").then_some(Build::Nix)
        );
        let bad = Manifest::parse(
            "[plugin]\nid = \"x\"\nname = \"X\"\nversion = \"1\"\napi = 1\n[backend]\nexec = \"x\"\nneeds = [\"/usr/bin/python3\"]\n",
        );
        assert!(bad.unwrap_err().contains("command names"));
        assert!(Manifest::parse(
            "[plugin]\nid = \"x\"\nname = \"X\"\nversion = \"1\"\napi = 1\n[backend]\nexec = \"x\"\nkind = \"cobol\"\n",
        )
        .is_err());
    }

    #[test]
    fn a_wrong_id_or_ref_is_explained() {
        let root = scratch("wrong");
        let (repo, _, _) = repository(&root);
        let locations = Locations {
            list: root.join("plugins.toml"),
            lock: root.join("plugins.lock"),
            bento: root.join("bento.toml"),
            installs: root.join("installs"),
        };
        let mut confirm = |_: &Plan| true;
        let mut installer = Installer {
            locations: &locations,
            confirm: &mut confirm,
            fetch: &Fixtures::default(),
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

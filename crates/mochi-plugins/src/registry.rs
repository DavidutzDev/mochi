//! Bento's registry: an index of plugins, themes and bentos whose releases
//! someone reviewed, each pinned to a commit.
//!
//! The registry is a git repository with a file per package,
//! `packages/<id>.toml`:
//!
//! ```toml
//! kind = "plugin"
//! name = "Pomodoro"
//! description = "A focus timer"
//! repository = "https://github.com/User/mochi-pomodoro"
//! maintainers = ["User"]
//! license = "MIT"
//! tags = ["productivity", "widget"]
//!
//! [[release]]
//! version = "0.3.0"
//! commit = "4f1c2a9d0e7b5a3c1d2e3f4a5b6c7d8e9f0a1b2c"
//! mochi = "0.0.7"
//! ```
//!
//! Its CI checks every file and publishes them together as `index.json`,
//! which Mochi downloads and keeps in `$XDG_CACHE_HOME/mochi/bento/`. The
//! code stays in each package's own repository; installing clones it at
//! the release's commit, the one a reviewer read.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use mochi_protocol::version;
use serde::{Deserialize, Serialize};

/// The registry Mochi knows without being told: its name in `bento:`
/// sources, and where its index is.
pub const DEFAULT: (&str, &str) = ("bento", "https://davidutzdev.github.io/bento/index.json");

/// The index's format. A newer one is refused, so an old Mochi says to
/// update instead of misreading it.
pub const FORMAT: u32 = 1;

/// How long a downloaded index counts as current.
const FRESH: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Plugin,
    Theme,
    Bento,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::Theme => "theme",
            Self::Bento => "bento",
        }
    }
}

/// `packages/<id>.toml`, and a package in `index.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub kind: Kind,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Where its code is: an `https://` URL `git clone` takes.
    pub repository: String,
    /// Who looks after it, by GitHub name.
    pub maintainers: Vec<String>,
    /// An SPDX license, like `MIT`.
    pub license: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// Pictures of it: paths in its repository in `packages/`, URLs in
    /// the index.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub screenshots: Vec<String>,
    #[serde(rename = "release")]
    pub releases: Vec<Release>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    /// The version its manifest says.
    pub version: String,
    /// The full commit a reviewer read.
    pub commit: String,
    /// The oldest Mochi it works with, as its manifest says.
    pub mochi: String,
    /// Why it shouldn't be installed any more. Installed ones keep
    /// working, with a warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yanked: Option<String>,
    /// Why it must not run: Mochi refuses to install it and stops it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub malicious: Option<String>,
}

impl Package {
    /// Checks what a reviewer can't see at a glance.
    pub fn check(&self, id: &str) -> Result<(), String> {
        crate::manifest::check_id(id)?;
        if self.name.trim().is_empty() {
            return Err("`name` is empty".into());
        }
        if !self.repository.starts_with("https://") {
            return Err(format!(
                "`repository` must be an https:// URL, not {:?}",
                self.repository
            ));
        }
        if self.maintainers.is_empty() {
            return Err("`maintainers` names nobody".into());
        }
        if self.license.trim().is_empty() {
            return Err("`license` is empty".into());
        }
        if self.releases.is_empty() {
            return Err("it has no [[release]]".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for release in &self.releases {
            let at = format!("[[release]] {}", release.version);
            version::parse(&release.version)
                .ok_or_else(|| format!("{at}: `version` isn't a version like \"1.0.0\""))?;
            version::parse(&release.mochi)
                .ok_or_else(|| format!("{at}: `mochi` isn't a version like \"0.0.8\""))?;
            let full = release.commit.len() == 40
                && release
                    .commit
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
            if !full {
                return Err(format!(
                    "{at}: `commit` must be a full commit, 40 lowercase hex digits"
                ));
            }
            if !seen.insert(version::parse(&release.version)) {
                return Err(format!("{at}: two releases have this version"));
            }
        }
        Ok(())
    }

    /// The release to install: `wanted`, or the newest this Mochi runs.
    /// Yanked and malicious ones are never picked.
    pub fn pick(&self, wanted: Option<&str>) -> Result<&Release, String> {
        let usable = |release: &&Release| release.yanked.is_none() && release.malicious.is_none();
        if let Some(wanted) = wanted {
            let release = self
                .releases
                .iter()
                .find(|release| version::parse(&release.version) == version::parse(wanted))
                .ok_or_else(|| format!("{} has no release {wanted}", self.name))?;
            if let Some(why) = release.malicious.as_ref().or(release.yanked.as_ref()) {
                return Err(format!("{} {wanted} was withdrawn: {why}", self.name));
            }
            version::supports(&release.mochi)
                .map_err(|error| format!("{} {wanted}: {error}", self.name))?;
            return Ok(release);
        }
        let mut candidates: Vec<&Release> = self.releases.iter().filter(usable).collect();
        candidates.sort_by_key(|release| version::parse(&release.version));
        let newest = candidates
            .last()
            .ok_or_else(|| format!("every release of {} was withdrawn", self.name))?;
        candidates
            .iter()
            .rev()
            .find(|release| version::supports(&release.mochi).is_ok())
            .copied()
            .ok_or_else(|| {
                format!(
                    "{} needs Mochi {} or newer, this is {}",
                    self.name,
                    newest.mochi,
                    version::VERSION
                )
            })
    }

    /// The release at `commit`, if the registry lists it.
    pub fn at(&self, commit: &str) -> Option<&Release> {
        self.releases
            .iter()
            .find(|release| release.commit == commit)
    }

    /// `owner/repo` for a GitHub repository.
    pub fn github(&self) -> Option<&str> {
        self.repository
            .strip_prefix("https://github.com/")
            .map(|rest| rest.trim_end_matches('/').trim_end_matches(".git"))
    }
}

/// `index.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub format: u32,
    pub packages: BTreeMap<String, Package>,
}

impl Index {
    pub fn parse(text: &str) -> Result<Self, String> {
        let index: Self = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if index.format > FORMAT {
            return Err(format!(
                "the registry's index is format {}, and this Mochi reads {FORMAT}: update Mochi",
                index.format
            ));
        }
        Ok(index)
    }

    pub fn get(&self, id: &str) -> Result<&Package, String> {
        self.packages
            .get(id)
            .ok_or_else(|| format!("the registry has no {id:?}"))
    }

    /// Packages whose id, name, description or tags contain `query`,
    /// ignoring case; every one without a query.
    pub fn search<'a>(
        &'a self,
        query: &str,
        kind: Option<Kind>,
    ) -> impl Iterator<Item = (&'a String, &'a Package)> {
        let query = query.to_lowercase();
        self.packages.iter().filter(move |(id, package)| {
            kind.is_none_or(|kind| package.kind == kind)
                && (query.is_empty()
                    || id.contains(&query)
                    || package.name.to_lowercase().contains(&query)
                    || package.description.to_lowercase().contains(&query)
                    || package.tags.iter().any(|tag| tag.to_lowercase() == query))
        })
    }
}

/// A registry: its name in `bento:<name>/<id>`, and its index's URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub name: String,
    pub url: String,
}

/// The registries: the default one, then the ones `bento.toml` adds in
/// `[registries]`, which can also point the default elsewhere.
pub fn registries(locations: &crate::Locations) -> Result<Vec<Registry>, String> {
    let installed = crate::bento::Installed::load(&locations.bento).map_err(|e| e.to_string())?;
    let mut list = vec![Registry {
        name: DEFAULT.0.to_owned(),
        url: DEFAULT.1.to_owned(),
    }];
    for (name, url) in installed.registries {
        match list.iter_mut().find(|registry| registry.name == name) {
            Some(registry) => registry.url = url,
            None => list.push(Registry { name, url }),
        }
    }
    Ok(list)
}

/// A registry by name, the default one without.
pub fn registry(locations: &crate::Locations, name: Option<&str>) -> Result<Registry, String> {
    let name = name.unwrap_or(DEFAULT.0);
    registries(locations)?
        .into_iter()
        .find(|registry| registry.name == name)
        .ok_or_else(|| format!("no registry called {name:?}; add it to [registries] in bento.toml"))
}

/// A registry's index: the copy kept in `cache` while it's fresh, or a new
/// download, or the kept copy when the download fails.
// Only commands run in a terminal load an index, so it says there when it
// falls back to the kept copy.
#[allow(clippy::print_stderr)]
pub fn load(registry: &Registry, cache: &Path, refresh: bool) -> Result<Index, String> {
    let kept = cache.join(format!("{}.json", registry.name));
    let age = std::fs::metadata(&kept)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok());
    if !refresh
        && age.is_some_and(|age| age < FRESH)
        && let Ok(index) = read(&kept)
    {
        return Ok(index);
    }
    match download(&registry.url) {
        Ok(text) => {
            let index =
                Index::parse(&text).map_err(|error| format!("{}: {error}", registry.url))?;
            let _ = std::fs::create_dir_all(cache);
            let temporary = kept.with_extension("json.new");
            if std::fs::write(&temporary, &text).is_ok() {
                let _ = std::fs::rename(&temporary, &kept);
            }
            Ok(index)
        }
        Err(error) => match read(&kept) {
            Ok(index) => {
                eprintln!(
                    "mochi: can't reach the registry {} ({error}), using the copy from before",
                    registry.name
                );
                Ok(index)
            }
            Err(_) => Err(format!(
                "can't reach the registry {}: {error}",
                registry.name
            )),
        },
    }
}

/// The kept copy only, never downloading: for mochid, which checks at
/// start that no plugin it runs was withdrawn.
pub fn cached(registry: &Registry, cache: &Path) -> Option<Index> {
    read(&cache.join(format!("{}.json", registry.name))).ok()
}

fn read(path: &Path) -> Result<Index, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    Index::parse(&text)
}

/// An `https://` URL with curl, or a `file://` one, for registries on disk.
fn download(url: &str) -> Result<String, String> {
    if let Some(path) = url.strip_prefix("file://") {
        return std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"));
    }
    let output = std::process::Command::new("curl")
        .args(["-fsSL", "--proto", "=https", "--max-time", "20", url])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("cannot run curl: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// Why the registry withdrew what a `bento:` source installed at `commit`,
/// from the index kept in the cache, never downloading: `Err` when it
/// withdrew it as harmful, `Ok(Some)` when only yanked.
pub fn withdrawn(
    locations: &crate::Locations,
    source: &crate::Source,
    commit: &str,
) -> Result<Option<String>, String> {
    let crate::Source::Registry { registry, id, .. } = source else {
        return Ok(None);
    };
    let Ok(registry) = self::registry(locations, registry.as_deref()) else {
        return Ok(None);
    };
    let Some(index) = cached(&registry, &cache_dir()) else {
        return Ok(None);
    };
    let Some(release) = index
        .packages
        .get(id)
        .and_then(|package| package.at(commit))
    else {
        return Ok(None);
    };
    if let Some(why) = &release.malicious {
        return Err(format!(
            "the registry withdrew {id} {} as harmful: {why}",
            release.version
        ));
    }
    Ok(release
        .yanked
        .as_ref()
        .map(|why| format!("the registry withdrew {id} {}: {why}", release.version)))
}

/// Where registries' indexes are kept: `$XDG_CACHE_HOME/mochi/bento`.
pub fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("/nonexistent"))
        .join("mochi")
        .join("bento")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(releases: &[(&str, &str)]) -> Package {
        Package {
            kind: Kind::Plugin,
            name: "Clock".into(),
            description: String::new(),
            repository: "https://github.com/User/clock".into(),
            maintainers: vec!["User".into()],
            license: "MIT".into(),
            tags: vec!["time".into()],
            homepage: None,
            screenshots: Vec::new(),
            releases: releases
                .iter()
                .enumerate()
                .map(|(index, (version, mochi))| Release {
                    version: (*version).into(),
                    commit: format!("{index:0>40}"),
                    mochi: (*mochi).into(),
                    yanked: None,
                    malicious: None,
                })
                .collect(),
        }
    }

    #[test]
    fn the_newest_release_this_mochi_runs_is_picked() {
        let mut clock = package(&[("1.0.0", "0.0.1"), ("1.2.0", "0.0.1"), ("2.0.0", "99.0")]);
        assert_eq!(clock.pick(None).unwrap().version, "1.2.0");
        clock.releases[1].yanked = Some("it eats batteries".into());
        assert_eq!(clock.pick(None).unwrap().version, "1.0.0");
        assert!(
            clock
                .pick(Some("1.2.0"))
                .unwrap_err()
                .contains("eats batteries")
        );
        assert!(
            clock
                .pick(Some("2.0.0"))
                .unwrap_err()
                .contains("needs Mochi 99.0")
        );
        clock.releases[0].malicious = Some("it sends your files away".into());
        assert!(clock.pick(None).unwrap_err().contains("needs Mochi 99.0"));
    }

    #[test]
    fn package_files_are_checked() {
        let fine = package(&[("1.0.0", "0.0.1")]);
        assert_eq!(fine.check("clock"), Ok(()));
        let mut short = fine.clone();
        short.releases[0].commit = "4f1c2a9".into();
        assert!(short.check("clock").unwrap_err().contains("full commit"));
        let mut http = fine.clone();
        http.repository = "http://example.com/clock".into();
        assert!(http.check("clock").unwrap_err().contains("https://"));
        let twice = package(&[("1.0.0", "0.0.1"), ("1.0", "0.0.1")]);
        assert!(twice.check("clock").unwrap_err().contains("two releases"));
        assert!(fine.check("Clock").is_err());
        assert_eq!(fine.github(), Some("User/clock"));
    }

    #[test]
    fn the_index_reads_searches_and_refuses_newer_formats() {
        let index = Index {
            format: FORMAT,
            packages: BTreeMap::from([("clock".into(), package(&[("1.0.0", "0.0.1")]))]),
        };
        let text = serde_json::to_string(&index).unwrap();
        let read = Index::parse(&text).unwrap();
        assert_eq!(read, index);
        assert_eq!(read.search("TIME", None).count(), 1);
        assert_eq!(read.search("clo", Some(Kind::Theme)).count(), 0);
        assert!(
            Index::parse(&text.replace("\"format\":1", "\"format\":9"))
                .unwrap_err()
                .contains("update Mochi")
        );
    }
}

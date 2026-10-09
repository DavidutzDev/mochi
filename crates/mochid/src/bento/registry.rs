//! Bento's registry from the command line: `search` and `info` for
//! everyone, and `registry check`, `index` and `diff` for the registry's
//! own CI, which runs them on every pull request and on every merge.

use std::collections::BTreeMap;
use std::path::Path;

use mochi_plugins::Locations;
use mochi_plugins::registry::{self, FORMAT, Index, Kind, Package};

use super::apply;

/// `mochi bento search`.
pub fn search(config_file: &Path, query: &str, kind: Option<&str>) -> Result<(), String> {
    let kind = kind.map(parse_kind).transpose()?;
    let locations = Locations::beside(config_file);
    let registry = registry::registry(&locations, None)?;
    let index = registry::load(&registry, &registry::cache_dir(), false)?;
    let installed = mochi_plugins::bento::Installed::load(&locations.bento)
        .map_err(|error| error.to_string())?;
    let mut found = 0;
    for (id, package) in index.search(query, kind) {
        found += 1;
        let version = match package.pick(None) {
            Ok(release) => release.version.clone(),
            Err(_) => "needs a newer Mochi".into(),
        };
        let here = installed.plugins.contains_key(id)
            || installed.themes.contains_key(id)
            || installed.bentos.contains_key(id);
        println!(
            "{id:<20} {:<7} {version:<10} {}{}",
            package.kind.as_str(),
            if package.description.is_empty() {
                &package.name
            } else {
                &package.description
            },
            if here { "  (installed)" } else { "" }
        );
    }
    if found == 0 {
        println!("Nothing in the registry matches {query:?}.");
    }
    Ok(())
}

/// `mochi bento info`.
pub fn info(config_file: &Path, id: &str) -> Result<(), String> {
    let locations = Locations::beside(config_file);
    let registry = registry::registry(&locations, None)?;
    let index = registry::load(&registry, &registry::cache_dir(), false)?;
    let package = index.get(id)?;
    println!("{} ({id}), a {}", package.name, package.kind.as_str());
    if !package.description.is_empty() {
        println!("  {}", package.description);
    }
    println!(
        "  by {}, {}",
        package.maintainers.join(", "),
        package.license
    );
    if !package.tags.is_empty() {
        println!("  tags       {}", package.tags.join(", "));
    }
    println!("  repository {}", package.repository);
    if let Some(homepage) = &package.homepage {
        println!("  homepage   {homepage}");
    }
    let picked = package
        .pick(None)
        .ok()
        .map(|release| release.commit.clone());
    let mut releases = package.releases.clone();
    releases.sort_by_key(|release| mochi_core::version::parse(&release.version));
    for release in releases.iter().rev() {
        let note = if let Some(why) = &release.malicious {
            format!("withdrawn as harmful: {why}")
        } else if let Some(why) = &release.yanked {
            format!("withdrawn: {why}")
        } else if picked.as_ref() == Some(&release.commit) {
            "what `add` installs".into()
        } else if mochi_core::version::supports(&release.mochi).is_err() {
            format!("needs Mochi {}", release.mochi)
        } else {
            String::new()
        };
        println!("  {:<10} {} {note}", release.version, &release.commit[..10]);
    }
    println!("Install it with `mochi bento add {id}`.");
    Ok(())
}

/// `mochi bento catalog`: the registry's packages and what Bento
/// installed, as JSON, for the settings panel. A registry that can't be
/// reached gives an `error` and the installed list still.
pub fn catalog(config_file: &Path, refresh: bool) -> Result<(), String> {
    let locations = Locations::beside(config_file);
    let installed = mochi_plugins::bento::Installed::load(&locations.bento)
        .map_err(|error| error.to_string())?;
    let lock = mochi_plugins::Lock::load(&locations.lock).unwrap_or_default();
    let loaded = registry::registry(&locations, None)
        .and_then(|registry| registry::load(&registry, &registry::cache_dir(), refresh));
    let (index, error) = match loaded {
        Ok(index) => (Some(index), None),
        Err(error) => (None, Some(error)),
    };
    let here = |id: &str| {
        installed.plugins.contains_key(id)
            || installed.themes.contains_key(id)
            || installed.bentos.contains_key(id)
    };
    let packages: Vec<serde_json::Value> = index
        .iter()
        .flat_map(|index| &index.packages)
        .map(|(id, package)| {
            let picked = package.pick(None);
            serde_json::json!({
                "id": id,
                "kind": package.kind.as_str(),
                "name": package.name,
                "description": package.description,
                "repository": package.repository,
                "maintainers": package.maintainers,
                "license": package.license,
                "tags": package.tags,
                "homepage": package.homepage,
                "screenshots": package.screenshots,
                "version": picked.as_ref().ok().map(|release| release.version.clone()),
                "problem": picked.err(),
                "installed": here(id),
            })
        })
        .collect();
    let mut list = Vec::new();
    for (kind, entries) in [
        ("bento", &installed.bentos),
        ("theme", &installed.themes),
        ("plugin", &installed.plugins),
    ] {
        for (id, entry) in entries {
            let locked = lock.plugins.get(id);
            let (version, commit) = if kind == "plugin" {
                (
                    locked.and_then(|locked| locked.version.clone()),
                    locked.and_then(|locked| locked.commit.clone()),
                )
            } else {
                (entry.version.clone(), entry.revision.clone())
            };
            let source = entry.source.parse::<mochi_plugins::Source>().ok();
            let (withdrawn, harmful) = match (&source, &commit) {
                (Some(source), Some(commit)) => {
                    match registry::withdrawn(&locations, source, commit) {
                        Ok(note) => (note, false),
                        Err(note) => (Some(note), true),
                    }
                }
                _ => (None, false),
            };
            // A newer release, for what came from the registry. What a bento
            // brought follows its bento.
            let listed = match &source {
                _ if entry.by.is_some() => None,
                Some(mochi_plugins::Source::Registry {
                    id: package,
                    version,
                    ..
                }) => index
                    .as_ref()
                    .and_then(|index| index.packages.get(package))
                    .map(|package| (package, version.clone())),
                _ => None,
            };
            let update = listed.and_then(|(package, pinned)| {
                let newest = package.pick(pinned.as_deref()).ok()?;
                (commit.as_deref() != Some(newest.commit.as_str())).then(|| newest.version.clone())
            });
            let name = index
                .as_ref()
                .and_then(|index| index.packages.get(id))
                .map_or_else(|| id.clone(), |package| package.name.clone());
            list.push(serde_json::json!({
                "id": id,
                "kind": kind,
                "name": name,
                "version": version,
                "source": entry.source,
                "by": entry.by,
                "withdrawn": withdrawn,
                "harmful": harmful,
                "update": update,
            }));
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "error": error,
            "packages": packages,
            "installed": list,
        })
    );
    Ok(())
}

fn parse_kind(text: &str) -> Result<Kind, String> {
    match text {
        "plugin" | "plugins" | "widget" | "widgets" => Ok(Kind::Plugin),
        "theme" | "themes" => Ok(Kind::Theme),
        "bento" | "bentos" => Ok(Kind::Bento),
        other => Err(format!("unknown kind {other:?}: plugin, theme or bento")),
    }
}

/// Every `packages/<id>.toml` in a registry's repository, checked.
fn packages(dir: &Path) -> Result<BTreeMap<String, Package>, String> {
    let folder = dir.join("packages");
    let entries = std::fs::read_dir(&folder)
        .map_err(|error| format!("cannot read {}: {error}", folder.display()))?;
    let mut packages = BTreeMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let id = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let package: Package = mochi_core::toml::from_str(&text)
            .map_err(|error| format!("packages/{id}.toml: {error}"))?;
        package
            .check(&id)
            .map_err(|error| format!("packages/{id}.toml: {error}"))?;
        packages.insert(id, package);
    }
    Ok(packages)
}

/// `mochi bento registry check`: every package file, and with `fetch`,
/// the newest release of each package in `only` (all without), cloned and
/// read as `mochi bento add` would.
pub fn check(dir: &Path, fetch: bool, only: &[String]) -> Result<(), String> {
    let packages = packages(dir)?;
    println!("{} packages read", packages.len());
    if !fetch {
        return Ok(());
    }
    let mut failed = Vec::new();
    for (id, package) in &packages {
        if !only.is_empty() && !only.contains(id) {
            continue;
        }
        let Some(release) = newest(package) else {
            continue;
        };
        match verify(id, package, release) {
            Ok(note) => println!("{id} {}: fine{note}", release.version),
            Err(error) => {
                println!("{id} {}: {error}", release.version);
                failed.push(id.clone());
            }
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("{} failed: {}", failed.len(), failed.join(", ")))
    }
}

/// The newest release that isn't withdrawn.
fn newest(package: &Package) -> Option<&registry::Release> {
    package
        .releases
        .iter()
        .filter(|release| release.yanked.is_none() && release.malicious.is_none())
        .max_by_key(|release| mochi_core::version::parse(&release.version))
}

/// Clones a release and checks its files say what its package file does.
fn verify(id: &str, package: &Package, release: &registry::Release) -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!("mochi-registry-{id}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let result = (|| {
        mochi_plugins::install::clone(&package.repository, Some(&release.commit), &scratch)
            .map_err(|error| error.0)?;
        apply::verify_release(&scratch, id, package.kind, &release.version, &release.mochi)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// `mochi bento registry index`: `index.json` from the package files, with
/// screenshots as URLs at the newest release's commit.
pub fn index(dir: &Path, out: &Path) -> Result<(), String> {
    let mut packages = packages(dir)?;
    for package in packages.values_mut() {
        let commit = newest(package).map(|release| release.commit.clone());
        let base = match (package.github(), commit) {
            (Some(repo), Some(commit)) => {
                Some(format!("https://raw.githubusercontent.com/{repo}/{commit}"))
            }
            _ => None,
        };
        package.screenshots = match base {
            Some(base) => package
                .screenshots
                .iter()
                .map(|path| format!("{base}/{}", path.trim_start_matches('/')))
                .collect(),
            None => Vec::new(),
        };
    }
    let index = Index {
        format: FORMAT,
        packages,
    };
    let text = serde_json::to_string_pretty(&index).map_err(|error| error.to_string())?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(out, text)
        .map_err(|error| format!("cannot write {}: {error}", out.display()))?;
    println!(
        "wrote {} with {} packages",
        out.display(),
        index.packages.len()
    );
    Ok(())
}

/// What changed between two checkouts of the registry, for a pull request.
#[derive(Debug, Default, serde::Serialize)]
pub struct Changes {
    /// A plugin gained code: a person must review it.
    pub review: bool,
    /// The packages that changed, to check with `--fetch`.
    pub changed: Vec<String>,
    /// The plugin releases to build: their code is new.
    pub builds: Vec<Build>,
    /// Markdown for the pull request: each change, with a link to the
    /// code between the releases.
    pub summary: String,
}

/// A plugin release for CI to build with Nix.
#[derive(Debug, serde::Serialize)]
pub struct Build {
    pub id: String,
    pub repository: String,
    pub commit: String,
}

/// `mochi bento registry diff`. `author` is who opened the pull request:
/// changes to a package they don't look after need a review.
pub fn diff(base: &Path, head: &Path, author: Option<&str>, json: bool) -> Result<(), String> {
    let before = if base.join("packages").is_dir() {
        packages(base)?
    } else {
        BTreeMap::new()
    };
    let after = packages(head)?;
    let changes = compare(&before, &after, author);
    if json {
        println!(
            "{}",
            serde_json::to_string(&changes).map_err(|error| error.to_string())?
        );
    } else {
        print!("{}", changes.summary);
    }
    Ok(())
}

/// What changed, and why a person must review it, if they must: a plugin
/// brings new code or moves elsewhere, a package leaves, changes hands or
/// moves, or someone changes a package they don't look after. The rest,
/// new themes and bentos and their maintainers' releases, merges itself.
fn compare(
    before: &BTreeMap<String, Package>,
    after: &BTreeMap<String, Package>,
    author: Option<&str>,
) -> Changes {
    let mut changes = Changes::default();
    let mut lines = Vec::new();
    let mut reasons = Vec::new();
    for (id, package) in after {
        let old = before.get(id);
        if old == Some(package) {
            continue;
        }
        changes.changed.push(id.clone());
        let kind = package.kind.as_str();
        let added: Vec<&registry::Release> = package
            .releases
            .iter()
            .filter(|release| old.is_none_or(|old| old.at(&release.commit).is_none()))
            .collect();
        let code = package.kind == Kind::Plugin;
        if code && !added.is_empty() {
            reasons.push(format!("the plugin `{id}` brings new code"));
        }
        if let Some(old) = old {
            if old.repository != package.repository {
                reasons.push(format!("`{id}` moves to {}", package.repository));
            }
            if old.maintainers != package.maintainers {
                reasons.push(format!("`{id}` changes maintainers"));
            }
            if old.kind != package.kind {
                reasons.push(format!("`{id}` changes kind"));
            }
            let theirs = author.is_some_and(|author| {
                old.maintainers
                    .iter()
                    .any(|maintainer| maintainer.eq_ignore_ascii_case(author))
            });
            if !theirs {
                reasons.push(format!("`{id}` changes, and its maintainers didn't ask"));
            }
        }
        match old {
            None => lines.push(format!(
                "- **New {kind}** `{id}`: {} from {}",
                package.name, package.repository
            )),
            Some(_) if added.is_empty() => {
                lines.push(format!("- `{id}`: its details or withdrawals changed"));
            }
            Some(_) => {}
        }
        for release in added {
            if code {
                changes.builds.push(Build {
                    id: id.clone(),
                    repository: package.repository.clone(),
                    commit: release.commit.clone(),
                });
            }
            let previous = old.and_then(newest);
            let link = match (package.github(), previous) {
                (Some(repo), Some(previous)) => format!(
                    " ([the code since {}](https://github.com/{repo}/compare/{}...{}))",
                    previous.version, previous.commit, release.commit
                ),
                (Some(repo), None) => format!(
                    " ([the code](https://github.com/{repo}/tree/{}))",
                    release.commit
                ),
                _ => String::new(),
            };
            lines.push(format!("- `{id}` {}{link}", release.version));
        }
    }
    for id in before.keys().filter(|id| !after.contains_key(*id)) {
        changes.changed.push(id.clone());
        lines.push(format!("- `{id}` leaves the registry"));
        reasons.push(format!("`{id}` leaves the registry"));
    }
    changes.review = !reasons.is_empty();
    let verdict = if changes.review {
        format!(
            "A maintainer of the registry reviews this before it merges: {}.",
            reasons.join("; ")
        )
    } else if changes.changed.is_empty() {
        "No package changed.".to_owned()
    } else {
        "Only new themes and bentos, or their maintainers' releases: this merges once the checks pass."
            .to_owned()
    };
    changes.summary = format!("{}\n\n{verdict}\n", lines.join("\n"));
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(kind: Kind, commits: &[&str]) -> Package {
        Package {
            kind,
            name: "X".into(),
            description: String::new(),
            repository: "https://github.com/User/x".into(),
            maintainers: vec!["User".into()],
            license: "MIT".into(),
            tags: Vec::new(),
            homepage: None,
            screenshots: Vec::new(),
            releases: commits
                .iter()
                .enumerate()
                .map(|(index, commit)| registry::Release {
                    version: format!("1.{index}.0"),
                    commit: commit.repeat(40),
                    mochi: "0.0.1".into(),
                    yanked: None,
                    malicious: None,
                })
                .collect(),
        }
    }

    #[test]
    fn new_plugin_code_needs_a_review_and_themes_dont() {
        let before = BTreeMap::from([
            ("clock".to_owned(), package(Kind::Plugin, &["a"])),
            ("dusk".to_owned(), package(Kind::Theme, &["b"])),
        ]);
        let mut after = before.clone();
        after.insert("dusk".into(), package(Kind::Theme, &["b", "c"]));
        let themes = compare(&before, &after, Some("user"));
        assert!(!themes.review);
        assert_eq!(themes.changed, ["dusk"]);

        after.insert("clock".into(), package(Kind::Plugin, &["a", "d"]));
        let plugins = compare(&before, &after, Some("user"));
        assert!(plugins.review);
        assert_eq!(plugins.builds.len(), 1);
        assert_eq!(plugins.builds[0].commit, "d".repeat(40));
        assert!(
            plugins
                .summary
                .contains(&format!("compare/{}...{}", "a".repeat(40), "d".repeat(40))),
            "{}",
            plugins.summary
        );

        let mut withdrawn = before.clone();
        withdrawn.get_mut("clock").unwrap().releases[0].yanked = Some("broken".into());
        assert!(!compare(&before, &withdrawn, Some("user")).review);

        // Someone else's package, a move, a new maintainer or a removal: a
        // person looks.
        assert!(compare(&before, &withdrawn, Some("stranger")).review);
        let mut moved = before.clone();
        moved.get_mut("dusk").unwrap().repository = "https://github.com/Other/dusk".into();
        assert!(compare(&before, &moved, Some("user")).review);
        let mut taken = before.clone();
        taken
            .get_mut("dusk")
            .unwrap()
            .maintainers
            .push("stranger".into());
        assert!(compare(&before, &taken, Some("user")).review);
        let mut gone = before.clone();
        gone.remove("dusk");
        assert!(compare(&before, &gone, Some("user")).review);
        // A new theme, from anyone, doesn't wait.
        let mut fresh = before.clone();
        fresh.insert("dawn".into(), package(Kind::Theme, &["e"]));
        assert!(!compare(&before, &fresh, Some("stranger")).review);
    }
}

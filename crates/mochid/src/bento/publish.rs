//! `mochi bento publish`: sends a package's release to the registry. Run
//! in the package's repository, it checks the release as the registry's
//! CI will, writes its `packages/<id>.toml`, or adds the release to the one
//! there, and opens the pull request with `gh`, from a fork of the
//! registry. `--print` only prints the file, to send by hand.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use mochi_plugins::registry::{Kind, Package, Release};

use super::apply;

/// Where the default registry takes pull requests.
pub const REPOSITORY: &str = "DavidutzDev/bento";

#[derive(Debug, Default)]
pub struct Options {
    /// The registry's repository, as `owner/name`.
    pub registry: Option<String>,
    pub tags: Vec<String>,
    /// An SPDX license, when the LICENSE file doesn't say.
    pub license: Option<String>,
    /// The oldest Mochi a plugin works with; themes and bentos say it in
    /// their manifest.
    pub mochi: Option<String>,
    pub print: bool,
    pub yes: bool,
}

/// What the package in a directory says about itself.
struct Listing {
    id: String,
    kind: Kind,
    name: String,
    description: String,
    version: String,
    mochi: String,
    screenshots: Vec<String>,
}

pub fn publish(dir: &Path, options: &Options) -> Result<(), String> {
    let dir = std::fs::canonicalize(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let git = |args: &[&str]| run("git", &dir, args);
    let commit = git(&["rev-parse", "HEAD"]).map_err(|_| {
        format!(
            "{} isn't a git repository with a commit: the registry pins commits",
            dir.display()
        )
    })?;
    if !git(&["status", "--porcelain"])?.is_empty() {
        return Err("commit your changes first: the registry pins what's committed".into());
    }
    let remote = git(&["config", "--get", "remote.origin.url"])
        .map_err(|_| "the repository has no `origin` remote to publish from".to_owned())?;
    let repository = https(&remote).ok_or_else(|| {
        format!("the registry needs an https repository, and `origin` is {remote}")
    })?;
    let published = git(&["ls-remote", "origin"])?;
    if !published.lines().any(|line| line.starts_with(&commit)) {
        return Err(format!(
            "push {} first, and tag it: the registry clones it from {repository}",
            &commit[..10]
        ));
    }

    let release = read(&dir, options)?;
    eprintln!(
        "Checking {} {} as the registry will…",
        release.id, release.version
    );
    apply::verify_release(
        &dir,
        &release.id,
        release.kind,
        &release.version,
        &release.mochi,
    )?;

    let license = match &options.license {
        Some(license) => license.clone(),
        None => license_of(&dir).ok_or(
            "no LICENSE file that says which license: name it with --license, like --license MIT",
        )?,
    };
    let entry = Release {
        version: release.version.clone(),
        commit: commit.clone(),
        mochi: release.mochi.clone(),
        yanked: None,
        malicious: None,
    };
    let fresh = |maintainer: String| Package {
        kind: release.kind,
        name: release.name.clone(),
        description: release.description.clone(),
        repository: repository.clone(),
        maintainers: vec![maintainer],
        license: license.clone(),
        tags: options.tags.clone(),
        homepage: None,
        screenshots: release.screenshots.clone(),
        releases: vec![entry.clone()],
    };

    if options.print {
        println!(
            "# packages/{}.toml, or its new [[release]] if it's there already",
            release.id
        );
        print!("{}", to_toml(&fresh("<your GitHub name>".into()))?);
        return Ok(());
    }

    let registry = options
        .registry
        .clone()
        .unwrap_or_else(|| REPOSITORY.to_owned());
    let gh = |args: &[&str]| run("gh", &dir, args);
    gh(&["auth", "status"]).map_err(|_| {
        "publishing opens a pull request with gh: install it and run `gh auth login`, or use --print"
            .to_owned()
    })?;
    let login = gh(&["api", "user", "--jq", ".login"])?;

    let scratch = std::env::temp_dir().join(format!("mochi-publish-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let result = (|| -> Result<Option<String>, String> {
        let url = format!("https://github.com/{registry}");
        run(
            "git",
            Path::new("/"),
            &[
                "clone",
                "--quiet",
                "--depth",
                "1",
                &url,
                &scratch.to_string_lossy(),
            ],
        )?;
        let file = scratch
            .join("packages")
            .join(format!("{}.toml", release.id));
        let (package, note) = match std::fs::read_to_string(&file) {
            Ok(text) => {
                let mut package: Package = mochi_core::toml::from_str(&text).map_err(|error| {
                    format!("the registry's packages/{}.toml: {error}", release.id)
                })?;
                add_release(&mut package, entry.clone())?;
                let theirs = package
                    .maintainers
                    .iter()
                    .any(|maintainer| maintainer.eq_ignore_ascii_case(&login));
                let note = if theirs {
                    ""
                } else {
                    "\nYou don't look after this package in the registry, so its maintainers or the registry's review it."
                };
                (package, note)
            }
            Err(_) => (fresh(login.clone()), ""),
        };
        package
            .check(&release.id)
            .map_err(|error| format!("packages/{}.toml: {error}", release.id))?;
        let text = to_toml(&package)?;
        eprintln!("\npackages/{}.toml:\n\n{text}", release.id);
        let review = if release.kind == Kind::Plugin {
            "A maintainer of the registry reads the plugin's code before it merges."
        } else {
            "A theme or a bento merges once the registry's checks pass."
        };
        eprintln!("{review}{note}");
        if !options.yes && !ask(&format!("Open a pull request on {registry}?")) {
            return Ok(None);
        }
        std::fs::create_dir_all(file.parent().expect("in packages/"))
            .map_err(|error| error.to_string())?;
        std::fs::write(&file, text).map_err(|error| error.to_string())?;

        let at = |args: &[&str]| run("git", &scratch, args);
        let branch = format!("bento/{}-{}", release.id, release.version);
        gh(&["repo", "fork", &registry, "--clone=false", "--remote=false"])?;
        let name = registry.rsplit('/').next().unwrap_or("bento");
        at(&[
            "remote",
            "add",
            "fork",
            &format!("https://github.com/{login}/{name}"),
        ])?;
        at(&["checkout", "--quiet", "-b", &branch])?;
        at(&["add", "packages"])?;
        let title = format!("{} {}", release.id, release.version);
        at(&["commit", "--quiet", "-m", &title])?;
        at(&[
            "-c",
            "credential.helper=",
            "-c",
            "credential.helper=!gh auth git-credential",
            "push",
            "--quiet",
            "--force",
            "fork",
            &format!("HEAD:{branch}"),
        ])?;
        let body = format!(
            "{} {} from {repository}, at {commit}.\n\nSent with `mochi bento publish`.",
            release.name, release.version
        );
        let url = gh(&[
            "pr",
            "create",
            "--repo",
            &registry,
            "--head",
            &format!("{login}:{branch}"),
            "--title",
            &title,
            "--body",
            &body,
        ])?;
        Ok(Some(url))
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    match result? {
        Some(url) => eprintln!("Opened {url}"),
        None => eprintln!("Nothing sent."),
    }
    Ok(())
}

/// The id, kind, name and version the directory's manifest says.
fn read(dir: &Path, options: &Options) -> Result<Listing, String> {
    if dir.join(super::manifest::FILE).is_file() {
        let bento = super::manifest::Bento::load(dir)?;
        let about = bento.bento;
        return Ok(Listing {
            id: about.id,
            kind: Kind::Bento,
            name: about.name,
            description: about.description,
            version: about.version,
            mochi: about.mochi,
            screenshots: about.screenshots,
        });
    }
    if dir.join(mochi_core::themes::MANIFEST).is_file() {
        let text = std::fs::read_to_string(dir.join(mochi_core::themes::MANIFEST))
            .map_err(|error| error.to_string())?;
        let theme = mochi_core::themes::ThemeFile::parse(&text)?.theme;
        return Ok(Listing {
            id: theme.id,
            kind: Kind::Theme,
            name: theme.name,
            description: theme.description,
            version: theme.version,
            mochi: theme.mochi,
            screenshots: Vec::new(),
        });
    }
    let manifest = mochi_plugins::Manifest::load(dir).map_err(|error| error.to_string())?;
    let plugin = manifest.plugin;
    Ok(Listing {
        id: plugin.id,
        kind: Kind::Plugin,
        name: plugin.name,
        description: plugin.description,
        version: plugin.version,
        mochi: options
            .mochi
            .clone()
            .unwrap_or_else(|| mochi_core::version::VERSION.to_owned()),
        screenshots: Vec::new(),
    })
}

/// A new release at the end of the package's, refused when its version is
/// listed already.
fn add_release(package: &mut Package, release: Release) -> Result<(), String> {
    let version = mochi_core::version::parse(&release.version);
    if package
        .releases
        .iter()
        .any(|listed| mochi_core::version::parse(&listed.version) == version)
    {
        return Err(format!(
            "the registry lists {} {} already: raise the version for a new release",
            package.name, release.version
        ));
    }
    package.releases.push(release);
    Ok(())
}

fn to_toml(package: &Package) -> Result<String, String> {
    mochi_core::toml::to_string_pretty(package).map_err(|error| error.to_string())
}

/// `origin` as the https URL the registry takes: `git@github.com:o/r.git`
/// and `ssh://git@host/o/r` become `https://github.com/o/r`.
fn https(remote: &str) -> Option<String> {
    let remote = remote.trim().trim_end_matches('/').trim_end_matches(".git");
    if remote.starts_with("https://") {
        return Some(remote.to_owned());
    }
    if let Some(rest) = remote.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return Some(format!("https://{host}/{path}"));
    }
    if let Some(rest) = remote.strip_prefix("ssh://") {
        let rest = rest.split_once('@').map_or(rest, |(_, rest)| rest);
        return Some(format!("https://{rest}"));
    }
    None
}

/// The SPDX id of the license in LICENSE, LICENCE or COPYING, for the
/// common ones.
fn license_of(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let file: PathBuf = entries.flatten().map(|entry| entry.path()).find(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_uppercase())
            .is_some_and(|name| {
                ["LICENSE", "LICENCE", "COPYING"]
                    .iter()
                    .any(|stem| name.starts_with(stem))
            })
    })?;
    let text = std::fs::read_to_string(file).ok()?;
    let head: String = text.chars().take(1200).collect::<String>().to_lowercase();
    let has = |words: &str| head.contains(words);
    let id = if has("mit license") || has("permission is hereby granted, free of charge") {
        "MIT"
    } else if has("apache license") && has("version 2.0") {
        "Apache-2.0"
    } else if has("gnu lesser general public license") && has("version 3") {
        "LGPL-3.0-or-later"
    } else if has("gnu general public license") && has("version 3") {
        "GPL-3.0-or-later"
    } else if has("gnu general public license") && has("version 2") {
        "GPL-2.0-or-later"
    } else if has("mozilla public license") && has("2.0") {
        "MPL-2.0"
    } else if has("free and unencumbered software released into the public domain") {
        "Unlicense"
    } else if has("permission to use, copy, modify, and/or distribute this software") {
        "ISC"
    } else if has("redistribution and use in source and binary forms") {
        "BSD-3-Clause"
    } else {
        return None;
    };
    Some(id.to_owned())
}

/// Runs a program in `dir` and returns what it printed, trimmed.
fn run(program: &str, dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let said = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{program} {}: {}",
            args.first().unwrap_or(&""),
            said.trim()
        ))
    }
}

fn ask(question: &str) -> bool {
    use std::io::Write as _;
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    let _ = std::io::stdin().read_line(&mut answer);
    matches!(answer.trim(), "y" | "Y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remotes_become_https() {
        assert_eq!(
            https("git@github.com:someone/mochi-dusk.git").as_deref(),
            Some("https://github.com/someone/mochi-dusk")
        );
        assert_eq!(
            https("ssh://git@codeberg.org/someone/dusk").as_deref(),
            Some("https://codeberg.org/someone/dusk")
        );
        assert_eq!(
            https("https://github.com/someone/dusk/").as_deref(),
            Some("https://github.com/someone/dusk")
        );
        assert_eq!(https("/home/someone/dusk"), None);
    }

    #[test]
    fn licenses_are_recognized() {
        let dir = std::env::temp_dir().join(format!("mochi-license-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(license_of(&dir), None);
        std::fs::write(
            dir.join("LICENSE"),
            "MIT License\n\nCopyright (c) 2026 Someone",
        )
        .unwrap();
        assert_eq!(license_of(&dir).as_deref(), Some("MIT"));
        std::fs::write(
            dir.join("LICENSE"),
            "GNU GENERAL PUBLIC LICENSE\nVersion 3, 29 June 2007",
        )
        .unwrap();
        assert_eq!(license_of(&dir).as_deref(), Some("GPL-3.0-or-later"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_release_is_added_once() {
        let release = |version: &str| Release {
            version: version.into(),
            commit: "a".repeat(40),
            mochi: "0.0.7".into(),
            yanked: None,
            malicious: None,
        };
        let mut package = Package {
            kind: Kind::Theme,
            name: "Dusk".into(),
            description: String::new(),
            repository: "https://github.com/someone/dusk".into(),
            maintainers: vec!["someone".into()],
            license: "MIT".into(),
            tags: Vec::new(),
            homepage: None,
            screenshots: Vec::new(),
            releases: vec![release("1.0.0")],
        };
        add_release(&mut package, release("1.1.0")).unwrap();
        assert_eq!(package.releases.len(), 2);
        assert!(
            add_release(&mut package, release("1.1"))
                .unwrap_err()
                .contains("already")
        );
        let text = to_toml(&package).unwrap();
        assert!(text.contains("[[release]]"), "{text}");
        let back: Package = mochi_core::toml::from_str(&text).unwrap();
        assert_eq!(back, package);
    }
}

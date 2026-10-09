//! Where a bento, a theme or a plugin comes from, and getting it: a
//! directory as it is, or a git repository cloned to a scratch directory,
//! which runs none of its code.

use std::path::{Path, PathBuf};

use mochi_plugins::Source;

/// What `mochi bento add` was given, as a source plugins.toml would take:
/// `git:`, `git-release:` and `path:` sources, a directory, a URL like a
/// gist's, `github.com/<user>/<repo>`, or `gh:<user>/<repo>`.
pub fn parse(text: &str) -> Result<Source, String> {
    let text = text.trim();
    if text.starts_with("git:") || text.starts_with("git-release:") {
        return text.parse().map_err(|error| format!("{error}"));
    }
    if let Some(path) = text.strip_prefix("path:") {
        return local(path);
    }
    if let Some(repo) = text.strip_prefix("gh:") {
        return format!("git:github.com/{repo}")
            .parse()
            .map_err(|error| format!("{error}"));
    }
    let looks_remote = text.contains("://")
        || [
            "github.com/",
            "gitlab.com/",
            "codeberg.org/",
            "gist.github.com/",
        ]
        .iter()
        .any(|host| text.starts_with(host));
    if looks_remote && !Path::new(text).exists() {
        return format!("git:{text}")
            .parse()
            .map_err(|error| format!("{error}"));
    }
    local(text)
}

/// A directory, made absolute so the record still finds it from anywhere.
fn local(text: &str) -> Result<Source, String> {
    let path = match text.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|home| Path::new(&home).join(rest))
            .ok_or("HOME isn't set")?,
        None => PathBuf::from(text),
    };
    let path =
        std::fs::canonicalize(&path).map_err(|error| format!("no directory at {text}: {error}"))?;
    if !path.is_dir() {
        return Err(format!("{text} isn't a directory"));
    }
    Ok(Source::Path(path))
}

/// A source's files on disk. A clone is removed when this goes.
#[derive(Debug)]
pub struct Fetched {
    pub dir: PathBuf,
    /// The commit, for a git source.
    pub revision: Option<String>,
    scratch: Option<PathBuf>,
}

impl Drop for Fetched {
    fn drop(&mut self) {
        if let Some(scratch) = &self.scratch {
            let _ = std::fs::remove_dir_all(scratch);
        }
    }
}

/// Gets a source's files: a directory in place, or a clone. A release's
/// repository is cloned at its tag, for its manifest; the plugin installer
/// downloads the release itself.
pub fn fetch(source: &Source) -> Result<Fetched, String> {
    let (url, reference) = match source {
        Source::Path(dir) => {
            return Ok(Fetched {
                dir: dir.clone(),
                revision: None,
                scratch: None,
            });
        }
        Source::Git { url, reference } => (url.clone(), reference.clone()),
        Source::GitRelease { owner, repo, tag } => {
            (format!("https://github.com/{owner}/{repo}"), tag.clone())
        }
    };
    let scratch = std::env::temp_dir().join(format!(
        "mochi-bento-{}-{}",
        std::process::id(),
        SCRATCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    let clone = scratch.join("repository");
    std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
    let mut fetched = Fetched {
        dir: clone.clone(),
        revision: None,
        scratch: Some(scratch),
    };
    eprintln!("Fetching {source}…");
    let commit = mochi_plugins::install::clone(&url, reference.as_deref(), &clone)
        .map_err(|error| error.0)?;
    fetched.revision = Some(commit);
    Ok(fetched)
}

static SCRATCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorthands_become_sources() {
        let text = |input: &str| parse(input).unwrap().to_string();
        assert_eq!(text("gh:User/cozy"), "git:github.com/User/cozy");
        assert_eq!(text("github.com/User/cozy"), "git:github.com/User/cozy");
        assert_eq!(
            text("https://gist.github.com/User/0123abcd"),
            "git:gist.github.com/User/0123abcd"
        );
        assert_eq!(
            text("git:github.com/User/cozy:v1"),
            "git:github.com/User/cozy:v1"
        );
        assert_eq!(
            text("git-release:github.com/User/clock:v2"),
            "git-release:github.com/User/clock:v2"
        );
        let dir = std::env::temp_dir();
        assert!(matches!(parse(dir.to_str().unwrap()), Ok(Source::Path(_))));
        assert!(
            parse("/no/such/bento")
                .unwrap_err()
                .contains("no directory")
        );
    }
}

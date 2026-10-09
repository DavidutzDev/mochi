//! Where a plugin comes from, as written in plugins.toml.
//!
//! - `git:github.com/User/Repo:main` clones the repository and builds the
//!   plugin from source. The part after the last colon is a branch, a tag
//!   or a commit; without it, the default branch. A URL with a scheme
//!   works too: `git:https://codeberg.org/User/Repo:v1`.
//! - `git-release:github.com/User/Repo:v2` downloads the release asset the
//!   manifest names, already built. Without a tag, the latest release.
//! - `path:~/code/my-plugin` uses a directory where it is.
//! - `bento:pomodoro` installs the newest release in Bento's registry that
//!   this Mochi runs, at the commit a reviewer read. `bento:pomodoro:0.3.0`
//!   picks a release, and `bento:friends/pomodoro` another registry.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Git {
        /// Something `git clone` takes.
        url: String,
        reference: Option<String>,
    },
    GitRelease {
        owner: String,
        repo: String,
        tag: Option<String>,
    },
    Path(PathBuf),
    /// A package in a Bento registry.
    Registry {
        /// The registry's name, the default one without.
        registry: Option<String>,
        id: String,
        version: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("source {text:?}: {message}")]
pub struct SourceError {
    text: String,
    message: String,
}

impl FromStr for Source {
    type Err = SourceError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = |message: &str| SourceError {
            text: text.to_owned(),
            message: message.to_owned(),
        };
        if let Some(path) = text.strip_prefix("path:") {
            if path.is_empty() {
                return Err(error("the path is empty"));
            }
            return Ok(Self::Path(PathBuf::from(path)));
        }
        if let Some(rest) = text.strip_prefix("git-release:") {
            let (location, tag) = split_reference(rest);
            let parts: Vec<&str> = location.trim_end_matches('/').split('/').collect();
            return match parts.as_slice() {
                ["github.com", owner, repo] if !owner.is_empty() && !repo.is_empty() => {
                    Ok(Self::GitRelease {
                        owner: (*owner).to_owned(),
                        repo: repo.trim_end_matches(".git").to_owned(),
                        tag,
                    })
                }
                _ => Err(error(
                    "git-release: takes github.com/<owner>/<repo>, optionally with :<tag>",
                )),
            };
        }
        if let Some(rest) = text.strip_prefix("bento:") {
            let (path, version) = match rest.split_once(':') {
                Some((path, version)) if !version.is_empty() => (path, Some(version.to_owned())),
                Some((path, _)) => (path, None),
                None => (rest, None),
            };
            let (registry, id) = match path.split_once('/') {
                Some((registry, id)) => (Some(registry.to_owned()), id),
                None => (None, path),
            };
            crate::manifest::check_id(id).map_err(|message| error(&message))?;
            return Ok(Self::Registry {
                registry,
                id: id.to_owned(),
                version,
            });
        }
        if let Some(rest) = text.strip_prefix("git:") {
            let (location, reference) = split_reference(rest);
            if location.is_empty() {
                return Err(error("the repository is empty"));
            }
            let url = if location.contains("://") {
                location
            } else {
                format!("https://{location}")
            };
            return Ok(Self::Git { url, reference });
        }
        Err(error("start it with git:, git-release:, path: or bento:"))
    }
}

/// `host/path:ref` into the location and the ref. A scheme's colon, as in
/// `https://`, isn't a ref.
fn split_reference(text: &str) -> (String, Option<String>) {
    let (scheme, rest) = match text.find("://") {
        Some(index) => text.split_at(index + 3),
        None => ("", text),
    };
    match rest.rsplit_once(':') {
        Some((location, reference)) if !reference.is_empty() => {
            (format!("{scheme}{location}"), Some(reference.to_owned()))
        }
        Some((location, _)) => (format!("{scheme}{location}"), None),
        None => (text.to_owned(), None),
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git { url, reference } => {
                let url = url.strip_prefix("https://").unwrap_or(url);
                write!(f, "git:{url}")?;
                if let Some(reference) = reference {
                    write!(f, ":{reference}")?;
                }
                Ok(())
            }
            Self::GitRelease { owner, repo, tag } => {
                write!(f, "git-release:github.com/{owner}/{repo}")?;
                if let Some(tag) = tag {
                    write!(f, ":{tag}")?;
                }
                Ok(())
            }
            Self::Path(path) => write!(f, "path:{}", path.display()),
            Self::Registry {
                registry,
                id,
                version,
            } => {
                f.write_str("bento:")?;
                if let Some(registry) = registry {
                    write!(f, "{registry}/")?;
                }
                f.write_str(id)?;
                if let Some(version) = version {
                    write!(f, ":{version}")?;
                }
                Ok(())
            }
        }
    }
}

impl Source {
    /// A `path:` source's directory: `~` is the home directory, and a
    /// relative path starts next to plugins.toml.
    pub fn path(&self, beside: &Path) -> Option<PathBuf> {
        let Self::Path(path) = self else {
            return None;
        };
        if let Ok(rest) = path.strip_prefix("~")
            && let Some(home) = std::env::var_os("HOME")
        {
            return Some(PathBuf::from(home).join(rest));
        }
        Some(beside.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Source {
        text.parse().unwrap()
    }

    #[test]
    fn registry_sources() {
        assert_eq!(
            parse("bento:pomodoro"),
            Source::Registry {
                registry: None,
                id: "pomodoro".into(),
                version: None,
            }
        );
        let pinned = parse("bento:friends/pomodoro:0.3.0");
        assert_eq!(
            pinned,
            Source::Registry {
                registry: Some("friends".into()),
                id: "pomodoro".into(),
                version: Some("0.3.0".into()),
            }
        );
        assert_eq!(pinned.to_string(), "bento:friends/pomodoro:0.3.0");
        assert!("bento:Pomodoro".parse::<Source>().is_err());
    }

    #[test]
    fn git_sources() {
        assert_eq!(
            parse("git:github.com/User/my-plugin:v2"),
            Source::Git {
                url: "https://github.com/User/my-plugin".into(),
                reference: Some("v2".into()),
            }
        );
        assert_eq!(
            parse("git:github.com/User/my-plugin"),
            Source::Git {
                url: "https://github.com/User/my-plugin".into(),
                reference: None,
            }
        );
        assert_eq!(
            parse("git:https://codeberg.org/User/x:feature/new"),
            Source::Git {
                url: "https://codeberg.org/User/x".into(),
                reference: Some("feature/new".into()),
            }
        );
        assert_eq!(
            parse("git:file:///tmp/repo"),
            Source::Git {
                url: "file:///tmp/repo".into(),
                reference: None,
            }
        );
    }

    #[test]
    fn release_sources() {
        assert_eq!(
            parse("git-release:github.com/User/weather:v0.2.0"),
            Source::GitRelease {
                owner: "User".into(),
                repo: "weather".into(),
                tag: Some("v0.2.0".into()),
            }
        );
        assert_eq!(
            parse("git-release:github.com/User/weather"),
            Source::GitRelease {
                owner: "User".into(),
                repo: "weather".into(),
                tag: None,
            }
        );
        assert!(
            "git-release:gitlab.com/User/weather"
                .parse::<Source>()
                .is_err()
        );
    }

    #[test]
    fn path_sources() {
        let source = parse("path:plugins/mine");
        assert_eq!(
            source.path(Path::new("/home/me/.config/mochi")),
            Some(PathBuf::from("/home/me/.config/mochi/plugins/mine"))
        );
        let absolute = parse("path:/srv/plugin");
        assert_eq!(
            absolute.path(Path::new("/elsewhere")),
            Some(PathBuf::from("/srv/plugin"))
        );
    }

    #[test]
    fn sources_print_as_written() {
        for text in [
            "git:github.com/User/x:main",
            "git:github.com/User/x",
            "git:file:///tmp/repo:main",
            "git-release:github.com/User/x:v1",
            "path:~/code/x",
        ] {
            assert_eq!(parse(text).to_string(), text);
        }
    }

    #[test]
    fn unknown_kinds_are_refused() {
        let error = "https://github.com/User/x".parse::<Source>().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("git:, git-release:, path: or bento:")
        );
    }
}

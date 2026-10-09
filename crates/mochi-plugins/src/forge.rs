//! The forges `git-release:` sources download from, and what installing
//! reads from their APIs: the latest release's tag, the manifest at a tag,
//! and an asset's URL by its name.
//!
//! A source names its host, as in `git-release:codeberg.org/User/repo`.
//! github.com, codeberg.org, gitea.com and gitlab.com are known by name.
//! Any other host is asked which forge it is: a Forgejo or Gitea answers
//! `/api/v1/repos/<owner>/<repo>`, a GitLab `/api/v4/projects/<path>`. A
//! self-hosted forge can have any name, so its name says nothing, and the
//! repository has to be found there anyway.

use crate::install::{Fetch, InstallError};
use crate::manifest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    /// Forgejo and Gitea, which share their API. Codeberg runs Forgejo.
    Forgejo,
    GitLab,
}

impl Forge {
    /// The forge a well-known host runs.
    pub fn known(host: &str) -> Option<Self> {
        match host {
            "github.com" => Some(Self::GitHub),
            "codeberg.org" | "gitea.com" => Some(Self::Forgejo),
            "gitlab.com" => Some(Self::GitLab),
            _ => None,
        }
    }
}

/// A repository on a forge, for reading its releases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Releases {
    pub forge: Forge,
    host: String,
    /// `owner/repo`, or longer with a GitLab's subgroups.
    repo: String,
}

impl Releases {
    /// The repository `repo` on `host`, on the forge the host is known to
    /// run, or the one whose API has the repository.
    pub fn find(host: &str, repo: &str, fetch: &dyn Fetch) -> Result<Self, InstallError> {
        let releases = |forge| Self {
            forge,
            host: host.to_owned(),
            repo: repo.to_owned(),
        };
        if let Some(forge) = Forge::known(host) {
            return Ok(releases(forge));
        }
        // A Forgejo or Gitea repository is always owner/repo.
        if repo.split('/').count() == 2
            && let Ok(found) = json(fetch, &format!("https://{host}/api/v1/repos/{repo}"))
            && found["full_name"].is_string()
        {
            return Ok(releases(Forge::Forgejo));
        }
        if let Ok(found) = json(
            fetch,
            &format!("https://{host}/api/v4/projects/{}", encode(repo)),
        ) && found["path_with_namespace"].is_string()
        {
            return Ok(releases(Forge::GitLab));
        }
        Err(InstallError(format!(
            "neither a Forgejo, a Gitea nor a GitLab on {host} has a public repository {repo}"
        )))
    }

    /// The tag of the latest release, leaving out drafts and prereleases.
    pub fn latest(&self, fetch: &dyn Fetch) -> Result<String, InstallError> {
        let Self { host, repo, .. } = self;
        let url = match self.forge {
            Forge::GitHub => format!("https://api.github.com/repos/{repo}/releases/latest"),
            Forge::Forgejo => format!("https://{host}/api/v1/repos/{repo}/releases/latest"),
            // Newest first. An upcoming release is one whose date is still
            // ahead.
            Forge::GitLab => format!("https://{host}/api/v4/projects/{}/releases", encode(repo)),
        };
        let found = json(fetch, &url)?;
        let release = match self.forge {
            Forge::GitLab => found
                .as_array()
                .and_then(|releases| {
                    releases
                        .iter()
                        .find(|release| release["upcoming_release"] != true)
                })
                .ok_or_else(|| InstallError(format!("{url} lists no release")))?,
            Forge::GitHub | Forge::Forgejo => &found,
        };
        release["tag_name"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| InstallError(format!("{url} names no tag")))
    }

    /// Where the plugin's manifest is at `tag`.
    pub fn manifest_url(&self, tag: &str) -> String {
        let Self { host, repo, .. } = self;
        let file = manifest::FILE;
        match self.forge {
            Forge::GitHub => format!("https://raw.githubusercontent.com/{repo}/{tag}/{file}"),
            Forge::Forgejo => format!(
                "https://{host}/api/v1/repos/{repo}/raw/{file}?ref={}",
                encode(tag)
            ),
            Forge::GitLab => format!(
                "https://{host}/api/v4/projects/{}/repository/files/{file}/raw?ref={}",
                encode(repo),
                encode(tag)
            ),
        }
    }

    /// The URL of the asset called `name` in the release at `tag`. GitHub's
    /// follow from the names; the others' are in the release.
    pub fn asset_url(
        &self,
        tag: &str,
        name: &str,
        fetch: &dyn Fetch,
    ) -> Result<String, InstallError> {
        let Self { host, repo, .. } = self;
        let (url, assets) = match self.forge {
            Forge::GitHub => {
                return Ok(format!(
                    "https://github.com/{repo}/releases/download/{tag}/{name}"
                ));
            }
            Forge::Forgejo => {
                let url = format!(
                    "https://{host}/api/v1/repos/{repo}/releases/tags/{}",
                    encode(tag)
                );
                let release = json(fetch, &url)?;
                let assets = release["assets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|asset| {
                        Some((
                            asset["name"].as_str()?.to_owned(),
                            asset["browser_download_url"].as_str()?.to_owned(),
                        ))
                    })
                    .collect::<Vec<_>>();
                (url, assets)
            }
            // A GitLab release's assets are links, which can point
            // anywhere: the direct URL goes through the release.
            Forge::GitLab => {
                let url = format!(
                    "https://{host}/api/v4/projects/{}/releases/{}",
                    encode(repo),
                    encode(tag)
                );
                let release = json(fetch, &url)?;
                let assets = release["assets"]["links"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|link| {
                        let to = link["direct_asset_url"]
                            .as_str()
                            .or_else(|| link["url"].as_str())?;
                        Some((link["name"].as_str()?.to_owned(), to.to_owned()))
                    })
                    .collect::<Vec<_>>();
                (url, assets)
            }
        };
        if let Some((_, to)) = assets.iter().find(|(asset, _)| asset == name) {
            return Ok(to.clone());
        }
        let names: Vec<&str> = assets.iter().map(|(asset, _)| asset.as_str()).collect();
        Err(InstallError(if names.is_empty() {
            format!("the release {tag} has no assets, so no {name} ({url})")
        } else {
            format!(
                "the release {tag} has no asset {name}, only {} ({url})",
                names.join(", ")
            )
        }))
    }
}

fn json(fetch: &dyn Fetch, url: &str) -> Result<serde_json::Value, InstallError> {
    let text = fetch.text(url)?;
    serde_json::from_str(&text).map_err(|error| InstallError(format!("{url}: {error}")))
}

/// `text` as one part of a URL: everything but letters, digits and `-._~`
/// percent-encoded, slashes too, as GitLab wants a project's path.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::tests::Fixtures;

    fn find(host: &str, repo: &str, fetch: &Fixtures) -> Releases {
        Releases::find(host, repo, fetch).unwrap()
    }

    #[test]
    fn encodes_paths_and_tags() {
        assert_eq!(encode("Group/Sub/repo"), "Group%2FSub%2Frepo");
        assert_eq!(encode("v1.0.0+build~1"), "v1.0.0%2Bbuild~1");
    }

    #[test]
    fn github_needs_no_api_for_assets() {
        let fetch = Fixtures::default().with(
            "https://api.github.com/repos/User/clock/releases/latest",
            r#"{"tag_name": "v2.1.0", "draft": false}"#,
        );
        let releases = find("github.com", "User/clock", &fetch);
        assert_eq!(releases.latest(&fetch).unwrap(), "v2.1.0");
        assert_eq!(
            releases.manifest_url("v2.1.0"),
            "https://raw.githubusercontent.com/User/clock/v2.1.0/mochi-plugin.toml"
        );
        assert_eq!(
            releases
                .asset_url("v2.1.0", "clock-x86_64.tar.gz", &fetch)
                .unwrap(),
            "https://github.com/User/clock/releases/download/v2.1.0/clock-x86_64.tar.gz"
        );
    }

    #[test]
    fn reads_forgejo_releases() {
        // Trimmed from codeberg.org's answers.
        let fetch = Fixtures::default()
            .with(
                "https://codeberg.org/api/v1/repos/User/clock/releases/latest",
                r#"{"id": 7, "tag_name": "v1.2.0", "draft": false, "prerelease": false, "assets": []}"#,
            )
            .with(
                "https://codeberg.org/api/v1/repos/User/clock/releases/tags/v1.2.0",
                r#"{"tag_name": "v1.2.0", "assets": [
                    {"id": 1, "name": "clock-1.2.0-x86_64-linux.tar.gz", "size": 10,
                     "browser_download_url": "https://codeberg.org/User/clock/releases/download/v1.2.0/clock-1.2.0-x86_64-linux.tar.gz"},
                    {"id": 2, "name": "clock-1.2.0-x86_64-linux.tar.gz.asc", "size": 1,
                     "browser_download_url": "https://codeberg.org/User/clock/releases/download/v1.2.0/clock-1.2.0-x86_64-linux.tar.gz.asc"}
                ]}"#,
            );
        let releases = find("codeberg.org", "User/clock", &fetch);
        assert_eq!(releases.forge, Forge::Forgejo);
        assert_eq!(releases.latest(&fetch).unwrap(), "v1.2.0");
        assert_eq!(
            releases.manifest_url("v1.2.0"),
            "https://codeberg.org/api/v1/repos/User/clock/raw/mochi-plugin.toml?ref=v1.2.0"
        );
        assert_eq!(
            releases
                .asset_url("v1.2.0", "clock-1.2.0-x86_64-linux.tar.gz", &fetch)
                .unwrap(),
            "https://codeberg.org/User/clock/releases/download/v1.2.0/clock-1.2.0-x86_64-linux.tar.gz"
        );
        // A release built for one architecture says which it has.
        let error = releases
            .asset_url("v1.2.0", "clock-1.2.0-aarch64-linux.tar.gz", &fetch)
            .unwrap_err()
            .0;
        assert!(
            error.contains(
                "no asset clock-1.2.0-aarch64-linux.tar.gz, only clock-1.2.0-x86_64-linux.tar.gz,"
            ),
            "{error}"
        );
    }

    #[test]
    fn reads_gitlab_releases() {
        // Trimmed from gitlab.com's answers: newest first, links as assets.
        let fetch = Fixtures::default()
            .with(
                "https://gitlab.com/api/v4/projects/Group%2FSub%2Fclock/releases",
                r#"[
                    {"tag_name": "v3.0.0", "upcoming_release": true, "assets": {"links": []}},
                    {"tag_name": "v2.0.0", "upcoming_release": false, "assets": {"links": []}},
                    {"tag_name": "v1.0.0", "upcoming_release": false, "assets": {"links": []}}
                ]"#,
            )
            .with(
                "https://gitlab.com/api/v4/projects/Group%2FSub%2Fclock/releases/v2.0.0",
                r#"{"tag_name": "v2.0.0", "assets": {"count": 3, "sources": [
                    {"format": "zip", "url": "https://gitlab.com/Group/Sub/clock/-/archive/v2.0.0/clock-v2.0.0.zip"}
                ], "links": [
                    {"id": 1, "name": "clock-2.0.0-x86_64-linux.tar.gz",
                     "url": "https://gitlab.com/api/v4/projects/1/packages/generic/clock/2.0.0/clock-2.0.0-x86_64-linux.tar.gz",
                     "direct_asset_url": "https://gitlab.com/Group/Sub/clock/-/releases/v2.0.0/downloads/clock-2.0.0-x86_64-linux.tar.gz",
                     "link_type": "package"},
                    {"id": 2, "name": "clock-2.0.0-aarch64-linux.tar.gz",
                     "url": "https://example.org/clock-2.0.0-aarch64-linux.tar.gz",
                     "link_type": "other"}
                ]}}"#,
            );
        let releases = find("gitlab.com", "Group/Sub/clock", &fetch);
        assert_eq!(releases.forge, Forge::GitLab);
        assert_eq!(releases.latest(&fetch).unwrap(), "v2.0.0");
        assert_eq!(
            releases.manifest_url("v2.0.0"),
            "https://gitlab.com/api/v4/projects/Group%2FSub%2Fclock/repository/files/mochi-plugin.toml/raw?ref=v2.0.0"
        );
        assert_eq!(
            releases
                .asset_url("v2.0.0", "clock-2.0.0-x86_64-linux.tar.gz", &fetch)
                .unwrap(),
            "https://gitlab.com/Group/Sub/clock/-/releases/v2.0.0/downloads/clock-2.0.0-x86_64-linux.tar.gz"
        );
        // Without a direct URL, the link's own.
        assert_eq!(
            releases
                .asset_url("v2.0.0", "clock-2.0.0-aarch64-linux.tar.gz", &fetch)
                .unwrap(),
            "https://example.org/clock-2.0.0-aarch64-linux.tar.gz"
        );

        let empty = Fixtures::default().with(
            "https://gitlab.com/api/v4/projects/Group%2FSub%2Fclock/releases",
            "[]",
        );
        let error = releases.latest(&empty).unwrap_err().0;
        assert!(error.contains("lists no release"), "{error}");
    }

    #[test]
    fn asks_other_hosts_which_forge_they_run() {
        let forgejo = Fixtures::default().with(
            "https://git.example.org/api/v1/repos/User/clock",
            r#"{"id": 3, "full_name": "User/clock", "html_url": "https://git.example.org/User/clock"}"#,
        );
        assert_eq!(
            find("git.example.org", "User/clock", &forgejo).forge,
            Forge::Forgejo
        );

        // A GitLab sends its sign-in page for Forgejo's path.
        let gitlab = Fixtures::default()
            .with(
                "https://gitlab.example.org/api/v1/repos/User/clock",
                "<!DOCTYPE html><html>Sign in</html>",
            )
            .with(
                "https://gitlab.example.org/api/v4/projects/User%2Fclock",
                r#"{"id": 9, "path_with_namespace": "User/clock"}"#,
            );
        let found = find("gitlab.example.org", "User/clock", &gitlab);
        assert_eq!(found.forge, Forge::GitLab);
        assert_eq!(
            found.manifest_url("v1"),
            "https://gitlab.example.org/api/v4/projects/User%2Fclock/repository/files/mochi-plugin.toml/raw?ref=v1"
        );

        let error = Releases::find("git.example.org", "User/clock", &Fixtures::default())
            .unwrap_err()
            .0;
        assert!(
            error.contains("neither a Forgejo, a Gitea nor a GitLab on git.example.org"),
            "{error}"
        );
    }
}

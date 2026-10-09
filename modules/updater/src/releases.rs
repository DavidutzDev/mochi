//! Mochi's releases on GitHub: which is the latest, and the notes of each
//! one newer than the running version, which are its changelog section.

use std::cmp::Ordering;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The releases, newest first. `MOCHI_RELEASES_API` points elsewhere, for a
/// fork or a test, also at a `file://`.
pub const API: &str = "https://api.github.com/repos/DavidutzDev/mochi/releases?per_page=30";
/// Where a release's own page is, to check the links GitHub gives.
pub const PAGES: &str = "https://github.com/DavidutzDev/mochi/releases/";

/// A version like 0.0.8; a leading `v` and anything after `-` or `+` go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64, u64, u64);

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('v');
        let core = text.split(['-', '+']).next()?;
        let mut parts = core.split('.').map(|part| part.parse::<u64>().ok());
        let version = Self(
            parts.next()??,
            parts.next()??,
            parts.next().unwrap_or(Some(0))?,
        );
        parts.next().is_none().then_some(version)
    }
}

/// One release, as the page shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    /// The day it was published, like 2026-10-09.
    pub date: String,
    /// Its changelog section, in Markdown.
    pub notes: String,
    pub url: String,
}

/// What GitHub's API says of a release; the rest is left out.
#[derive(Debug, Deserialize)]
struct Raw {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// The published releases newer than `current`, newest first. Drafts,
/// prereleases and tags that aren't versions are left out.
pub fn newer(json: &str, current: &str) -> Result<Vec<Release>, String> {
    let raw: Vec<Raw> = serde_json::from_str(json)
        .map_err(|error| format!("GitHub's answer isn't a list of releases: {error}"))?;
    let current = Version::parse(current);
    let mut out: Vec<(Version, Release)> = raw
        .into_iter()
        .filter(|release| !release.draft && !release.prerelease)
        .filter_map(|release| {
            let version = Version::parse(&release.tag_name)?;
            let newer = current.is_none_or(|current| version.cmp(&current) == Ordering::Greater);
            newer.then(|| {
                (
                    version,
                    Release {
                        version: release.tag_name.trim_start_matches('v').to_owned(),
                        date: release
                            .published_at
                            .as_deref()
                            .and_then(|date| date.get(..10))
                            .unwrap_or_default()
                            .to_owned(),
                        notes: plain_headings(release.body.as_deref().unwrap_or_default()),
                        // Only Mochi's own pages open from here.
                        url: if release.html_url.starts_with(PAGES) {
                            release.html_url
                        } else {
                            String::new()
                        },
                    },
                )
            })
        })
        .collect();
    out.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
    Ok(out.into_iter().map(|(_, release)| release).collect())
}

/// The notes with their headings, like `### Added`, as bold lines, so they
/// stay under the release's own title on the page.
fn plain_headings(notes: &str) -> String {
    notes
        .trim()
        .lines()
        .map(|line| match line.trim_start().strip_prefix('#') {
            Some(rest) => format!("**{}**", rest.trim_start_matches('#').trim()),
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The list of releases, from GitHub or where `MOCHI_RELEASES_API` says.
pub async fn fetch() -> Result<String, String> {
    // A list named in the environment may be a file, for a test.
    let (url, protocols) = match std::env::var("MOCHI_RELEASES_API") {
        Ok(url) => (url, "=https,file"),
        Err(_) => (API.to_owned(), "=https"),
    };
    let run = tokio::process::Command::new("curl")
        .args([
            "-fsSL",
            "--proto",
            protocols,
            "--max-time",
            "20",
            "-H",
            "Accept: application/vnd.github+json",
            &url,
        ])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(30), run)
        .await
        .map_err(|_| "GitHub didn't answer in time".to_owned())?
        .map_err(|error| format!("can't run curl: {error}"))?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if error.is_empty() {
            "can't reach GitHub".to_owned()
        } else {
            format!("can't reach GitHub: {}", error.trim_start_matches("curl: "))
        });
    }
    String::from_utf8(output.stdout).map_err(|_| "GitHub's answer isn't text".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        let v = |text| Version::parse(text).unwrap();
        assert!(v("0.0.10") > v("0.0.9"));
        assert!(v("v0.1.0") > v("0.0.99"));
        assert_eq!(v("1.2"), v("1.2.0"));
        assert_eq!(v("0.0.9-rc1"), v("0.0.9"));
        assert!(Version::parse("latest").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
    }

    #[test]
    fn only_published_newer_releases_count() {
        let json = r#"[
            {"tag_name": "v0.0.10", "body": "- Ten", "published_at": "2026-11-02T10:00:00Z", "html_url": "https://github.com/DavidutzDev/mochi/releases/tag/v0.0.10"},
            {"tag_name": "v0.0.11", "body": "draft", "draft": true},
            {"tag_name": "v0.1.0-rc1", "prerelease": true},
            {"tag_name": "v0.0.9", "body": "- Nine\n", "published_at": "2026-10-20T10:00:00Z", "html_url": "https://elsewhere.example/v0.0.9"},
            {"tag_name": "v0.0.8", "body": "- Eight"},
            {"tag_name": "nightly"}
        ]"#;
        let newer = newer(json, "0.0.8").unwrap();
        let versions: Vec<&str> = newer
            .iter()
            .map(|release| release.version.as_str())
            .collect();
        assert_eq!(versions, ["0.0.10", "0.0.9"]);
        assert_eq!(newer[0].date, "2026-11-02");
        assert_eq!(newer[1].notes, "- Nine");
        assert_eq!(plain_headings("### Added\n\n- One"), "**Added**\n\n- One");
        assert_eq!(newer[1].url, "", "a link off Mochi's releases is dropped");
        assert!(super::newer(json, "0.0.10").unwrap().is_empty());
        assert!(super::newer("{}", "0.0.8").is_err());
    }
}

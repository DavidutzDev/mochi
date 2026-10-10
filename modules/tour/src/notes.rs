//! What each release changed, from `CHANGELOG.md` as it was when Mochi was
//! built, for a tour of what's new after a patch release: those bring
//! fixes and small changes, never tour steps of their own, so the tour
//! shows their changelog entries instead.

use crate::steps::Version;

/// The changelog this build came with.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// One entry of a release's changelog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub version: Version,
    /// The section it's under: `Added`, `Changed` or `Fixed`.
    pub kind: String,
    /// The entry, without its Markdown.
    pub text: String,
}

/// The entries of the releases after `seen` up to `current`, oldest first.
pub fn between(seen: Version, current: Version) -> Vec<Note> {
    let mut notes: Vec<Note> = parse(CHANGELOG)
        .into_iter()
        .filter(|note| note.version > seen && note.version <= current)
        .collect();
    notes.sort_by_key(|note| note.version);
    notes
}

/// Every entry of every dated release in `text`, a changelog in the Keep a
/// Changelog layout: `## 0.1.1 - 2026-10-11`, then `### Fixed` and its
/// `- ` entries. `Unreleased` is left out.
pub fn parse(text: &str) -> Vec<Note> {
    let mut notes = Vec::new();
    let mut version = None;
    let mut kind = String::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            version = heading.split_whitespace().next().and_then(Version::parse);
            kind.clear();
        } else if let Some(heading) = line.strip_prefix("### ") {
            kind = heading.trim().to_owned();
        } else if let (Some(version), Some(entry)) = (version, line.strip_prefix("- ")) {
            notes.push(Note {
                version,
                kind: kind.clone(),
                text: plain(entry),
            });
        }
    }
    notes
}

/// An entry without its Markdown: `code` and **bold** lose their marks,
/// and a [link](url) keeps its text.
fn plain(markdown: &str) -> String {
    let mut text = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest.find('[') {
        let Some(middle) = rest[start..].find("](") else {
            break;
        };
        let Some(end) = rest[start + middle..].find(')') else {
            break;
        };
        text.push_str(&rest[..start]);
        text.push_str(&rest[start + 1..start + middle]);
        rest = &rest[start + middle + end + 1..];
    }
    text.push_str(rest);
    text.replace('`', "").replace("**", "").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\n## Unreleased\n\n### Fixed\n\n- Not out yet.\n\n## 0.1.2 - 2026-10-12\n\n### Fixed\n\n- The `region` bar fits.\n\n## 0.1.1 - 2026-10-11\n\n### Added\n\n- Drag files, like [the docs](https://example.com) say.\n\n### Fixed\n\n- **Faster** panels.\n\n## 0.1.0 - 2026-10-10\n\n### Added\n\n- A clock.\n";

    #[test]
    fn reads_the_dated_releases_without_their_markdown() {
        let notes = parse(SAMPLE);
        let texts: Vec<(String, &str, &str)> = notes
            .iter()
            .map(|note| {
                (
                    note.version.to_string(),
                    note.kind.as_str(),
                    note.text.as_str(),
                )
            })
            .collect();
        assert_eq!(
            texts,
            [
                ("0.1.2".to_owned(), "Fixed", "The region bar fits."),
                (
                    "0.1.1".to_owned(),
                    "Added",
                    "Drag files, like the docs say."
                ),
                ("0.1.1".to_owned(), "Fixed", "Faster panels."),
                ("0.1.0".to_owned(), "Added", "A clock."),
            ]
        );
    }

    #[test]
    fn the_shipped_changelog_parses() {
        assert!(
            parse(CHANGELOG)
                .iter()
                .any(|note| note.version == Version(0, 1, 0))
        );
    }
}

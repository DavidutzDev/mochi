//! Ranks history entries for a query: newest first without one, otherwise
//! fuzzy matches on the start of the text, newest first among equals.
//! Images match on "image".

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::store::{Entry, Kind};

pub fn rank<'a>(entries: &'a [Entry], query: &str, limit: usize) -> Vec<&'a Entry> {
    let query = query.trim();
    if query.is_empty() {
        return entries.iter().take(limit).collect();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buffer = Vec::new();
    // Entries come newest first; a stable sort keeps that among equals.
    let mut scored: Vec<(u32, &Entry)> = entries
        .iter()
        .filter_map(|entry| {
            let text = match entry.kind {
                Kind::Text => entry.preview.as_str(),
                Kind::Image => "image",
            };
            pattern
                .score(Utf32Str::new(text, &mut buffer), &mut matcher)
                .map(|score| (score, entry))
        })
        .collect();
    scored.sort_by(|(a, _), (b, _)| b.cmp(a));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, entry)| entry)
        .collect()
}

#[cfg(test)]
mod tests {
    use zeroize::Zeroizing;

    use super::*;
    use crate::store::{Clip, Store};

    #[test]
    fn newest_first_and_fuzzy() {
        let dir =
            std::env::temp_dir().join(format!("mochi-clipboard-search-{}", std::process::id()));
        let mut store = Store::open(&dir.join("history"), None).unwrap();
        for (time, text) in [
            (1, "cargo build --release"),
            (2, "hello world"),
            (3, "git push"),
        ] {
            let clip = Clip {
                kind: Kind::Text,
                formats: vec![(
                    "text/plain".into(),
                    Zeroizing::new(text.as_bytes().to_vec()),
                )],
            };
            store.add(&clip, time).unwrap();
        }
        let previews = |query: &str| -> Vec<String> {
            rank(store.entries(), query, 10)
                .iter()
                .map(|entry| entry.preview.to_string())
                .collect()
        };
        assert_eq!(
            previews(""),
            ["git push", "hello world", "cargo build --release"]
        );
        assert_eq!(previews("hlo"), ["hello world"]);
        assert_eq!(previews("release")[0], "cargo build --release");
        assert!(previews("zzz").is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

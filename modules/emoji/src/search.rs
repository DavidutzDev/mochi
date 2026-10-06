//! The emoji table and the search over it. A query is words; an emoji
//! matches when every word starts a word of its name, its subgroup or its
//! group, or is the emoji itself. The panel's view searches the same way in
//! JavaScript, from the words the module publishes; keep the two in step.

use std::collections::HashSet;

use crate::recents::Recents;

/// Every emoji, made by `data/generate.sh`: emoji, name, group, subgroup.
pub const TABLE: &str = include_str!("../data/emoji.tsv");

/// Unicode's groups, in the table's order: the group as the table spells
/// it, its title, and the emoji on its tab.
pub const GROUPS: [(&str, &str, &str); 9] = [
    ("smileys & emotion", "Smileys & Emotion", "😀"),
    ("people & body", "People & Body", "👋"),
    ("animals & nature", "Animals & Nature", "🐶"),
    ("food & drink", "Food & Drink", "🍔"),
    ("travel & places", "Travel & Places", "🚗"),
    ("activities", "Activities", "⚽"),
    ("objects", "Objects", "💡"),
    ("symbols", "Symbols", "🔣"),
    ("flags", "Flags", "🏁"),
];

/// One line of the table.
#[derive(Debug)]
pub struct Emoji {
    pub glyph: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    /// The words of the name, lowercase, in order.
    pub name_words: Vec<String>,
    pub subgroup_words: Vec<String>,
    pub group_words: Vec<String>,
}

/// Reads the table. A bad line is an error, with the line.
pub fn parse(table: &'static str) -> Result<Vec<Emoji>, &'static str> {
    table
        .lines()
        // Comments start with "# ". The keycap "#️⃣" starts with "#" too.
        .filter(|line| !line.is_empty() && !line.starts_with("# "))
        .map(|line| {
            let mut fields = line.split('\t');
            let (Some(glyph), Some(name), Some(group), Some(subgroup), None) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) else {
                return Err(line);
            };
            if glyph.is_empty() || name.is_empty() {
                return Err(line);
            }
            Ok(Emoji {
                glyph,
                name,
                group,
                name_words: words(name),
                subgroup_words: words(subgroup),
                group_words: words(group),
            })
        })
        .collect()
}

/// The words of a name, lowercase: "flag: United States" has "flag",
/// "united" and "states".
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// How well a query word matches a word of an emoji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Match {
    Prefix,
    Exact,
}

fn match_word(query: &str, word: &str) -> Option<Match> {
    if word == query {
        return Some(Match::Exact);
    }
    if word.starts_with(query) {
        return Some(Match::Prefix);
    }
    // "smile" finds "smiling", "dance" finds "dancing".
    let stem = query.strip_suffix('e')?;
    word.strip_prefix(stem)?
        .starts_with("ing")
        .then_some(Match::Prefix)
}

/// The best match of a query word among some words.
fn best(query: &str, words: &[String]) -> Option<Match> {
    words
        .iter()
        .filter_map(|word| match_word(query, word))
        .max()
}

/// How well an emoji matches the query words, higher is better, or `None`
/// when a word matches nothing. Words of the name count most, then the
/// subgroup's, then the group's.
fn score(emoji: &Emoji, query: &[String]) -> Option<u32> {
    let mut total = 0;
    for word in query {
        // Points for an exact word and for a prefix, in each field.
        let points = |words: &[String], exact, prefix| {
            best(word, words).map(|found| if found == Match::Exact { exact } else { prefix })
        };
        total += if emoji.glyph == word || emoji.glyph.trim_end_matches('\u{fe0f}') == word {
            6
        } else {
            points(&emoji.name_words, 6, 4)
                .or_else(|| points(&emoji.subgroup_words, 3, 2))
                .or_else(|| points(&emoji.group_words, 2, 1))?
        };
    }
    let first = emoji.name_words.first()?;
    if query.iter().any(|word| match_word(word, first).is_some()) {
        total += 3;
    }
    if emoji.name_words == query {
        total += 10;
    }
    Some(total)
}

/// The emoji for a query, best first, at most `max`. Without words, the
/// recently picked ones, then the rest in the table's order, which starts
/// with smileys.
pub fn search<'a>(
    emoji: &'a [Emoji],
    query: &str,
    recents: &Recents,
    max: usize,
) -> Vec<&'a Emoji> {
    let query: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if query.is_empty() {
        let recent = recents
            .list()
            .iter()
            .filter_map(|id| emoji.iter().find(|emoji| emoji.glyph == id));
        let mut seen = HashSet::new();
        return recent
            .chain(emoji)
            .filter(|emoji| seen.insert(emoji.glyph))
            .take(max)
            .collect();
    }

    let mut found: Vec<(u32, usize, &Emoji)> = emoji
        .iter()
        .enumerate()
        .filter_map(|(index, emoji)| {
            let mut points = score(emoji, &query)?;
            if recents.contains(emoji.glyph) {
                points += 2;
            }
            Some((points, index, emoji))
        })
        .collect();
    // Higher scores first; then shorter names, which are the plainer
    // emoji, like "cat face" before "grinning cat"; then the table's order.
    found.sort_by_key(|&(points, index, emoji)| {
        (std::cmp::Reverse(points), emoji.name_words.len(), index)
    });
    found
        .into_iter()
        .take(max)
        .map(|(_, _, emoji)| emoji)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: usize = 50;

    fn table() -> Vec<Emoji> {
        parse(TABLE).expect("emoji.tsv parses")
    }

    fn glyphs(query: &str, recents: &Recents) -> Vec<&'static str> {
        search(&table(), query, recents, MAX)
            .iter()
            .map(|emoji| emoji.glyph)
            .collect()
    }

    fn find(query: &str) -> Vec<&'static str> {
        glyphs(query, &Recents::new(8))
    }

    #[test]
    fn the_table_parses() {
        let emoji = table();
        assert!(emoji.len() > 1500, "only {} emoji", emoji.len());
        assert!(emoji.iter().all(|emoji| !emoji.name_words.is_empty()));
        assert!(!emoji.iter().any(|emoji| emoji.name.contains("skin tone")));
        assert!(emoji.iter().any(|emoji| emoji.glyph == "#\u{fe0f}\u{20e3}"));
    }

    #[test]
    fn the_groups_come_in_the_tables_order() {
        let emoji = table();
        let mut order: Vec<&str> = emoji.iter().map(|emoji| emoji.group).collect();
        order.dedup();
        let groups: Vec<&str> = GROUPS.iter().map(|(group, _, _)| *group).collect();
        assert_eq!(order, groups);
        // Each tab's emoji is in its group.
        for (group, _, glyph) in GROUPS {
            let found = emoji.iter().find(|emoji| emoji.glyph == glyph);
            assert_eq!(found.map(|emoji| emoji.group), Some(group), "{glyph}");
        }
    }

    #[test]
    fn smile_finds_smiling_faces_first() {
        let found = find("smile");
        let emoji = table();
        let name = |glyph: &str| emoji.iter().find(|e| e.glyph == glyph).unwrap().name;
        for glyph in &found[..10] {
            assert!(name(glyph).contains("smiling"), "{} is early", name(glyph));
        }
        assert!(found.contains(&"😄"));
    }

    #[test]
    fn several_words_narrow_it_down() {
        assert_eq!(find("cat face")[0], "🐱");
        assert_eq!(find("heart red")[0], "❤\u{fe0f}");
        assert_eq!(find("Cat  FACE")[0], "🐱");
    }

    #[test]
    fn a_whole_name_comes_first() {
        assert_eq!(find("thumbs up")[0], "👍");
        assert_eq!(find("fire")[0], "🔥");
    }

    #[test]
    fn group_words_match_too() {
        // "flag" is the start of every flag's name; "flags" is the group.
        assert!(find("flags").len() == MAX);
        assert!(find("drink").contains(&"🍺"));
    }

    #[test]
    fn every_word_must_match() {
        assert!(find("zzzz").is_empty());
        assert!(find("cat zzzz").is_empty());
    }

    #[test]
    fn the_emoji_itself_finds_it() {
        assert_eq!(find("🐱")[0], "🐱");
        // Without its variation selector, as some keyboards type it.
        assert_eq!(find("❤")[0], "❤\u{fe0f}");
    }

    #[test]
    fn results_are_capped() {
        assert_eq!(find("face").len(), MAX);
        assert_eq!(search(&table(), "face", &Recents::new(8), 3).len(), 3);
    }

    #[test]
    fn recents_come_first_without_words() {
        let mut recents = Recents::new(8);
        recents.push("🐱");
        recents.push("❤\u{fe0f}");
        let found = glyphs("", &recents);
        assert_eq!(found[..3], ["❤\u{fe0f}", "🐱", "😀"]);
        assert_eq!(found.len(), MAX);
        assert_eq!(found.iter().filter(|glyph| **glyph == "🐱").count(), 1);
    }

    #[test]
    fn recents_rank_higher() {
        let first = find("grinning")[0];
        let other = find("grinning")[1];
        let mut recents = Recents::new(8);
        recents.push(other);
        assert_eq!(glyphs("grinning", &recents)[0], other);
        assert_ne!(first, other);
    }
}

//! An example Mochi plugin: an emoji picker for the launcher.
//!
//! Typing ":" in the launcher, then a few words, asks this plugin. It finds
//! the emoji whose names have those words, like "cat face" for 🐱, and
//! prints them as launcher results; Enter copies the emoji. The launcher
//! then tells it which one was picked, and it remembers the last few in a
//! file, to show them first next time.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use mochi_sdk::{ModuleCommand, ModuleCtx, ModuleEvent};
use serde::{Deserialize, Serialize};

/// Every emoji, made by `data/generate.sh`: emoji, name, group, subgroup.
const TABLE: &str = include_str!("../data/emoji.tsv");

/// The most results the launcher takes.
const MAX_RESULTS: usize = 50;

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    max_results: usize,
    recent: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_results: MAX_RESULTS,
            recent: 8,
        }
    }
}

/// One line of the table.
#[derive(Debug)]
struct Emoji {
    glyph: &'static str,
    name: &'static str,
    group: &'static str,
    /// The words of the name, lowercase, in order.
    name_words: Vec<String>,
    subgroup_words: Vec<String>,
    group_words: Vec<String>,
}

/// A line of the search action's output, as the launcher reads it.
#[derive(Debug, Serialize)]
struct Line<'a> {
    title: &'a str,
    subtitle: &'a str,
    glyph: &'a str,
    copy: &'a str,
    id: &'a str,
}

/// The emoji picked last, most recent first.
#[derive(Debug)]
struct Recents {
    list: Vec<String>,
    cap: usize,
}

struct Picker {
    emoji: Vec<Emoji>,
    recents: Recents,
    max_results: usize,
    /// Where the recents are kept. `None` without a home directory.
    file: Option<PathBuf>,
}

fn main() -> std::process::ExitCode {
    mochi_sdk::run(run)
}

async fn run(mut ctx: ModuleCtx) -> Result<(), mochi_sdk::Error> {
    let settings: Settings = ctx.settings()?;
    let file = recents_file();
    let mut recents = Recents::new(settings.recent);
    if let Some(file) = &file {
        recents.load(file);
    }
    let mut picker = Picker {
        emoji: parse(TABLE).map_err(|line| format!("bad line in emoji.tsv: {line}"))?,
        recents,
        max_results: settings.max_results.min(MAX_RESULTS),
        file,
    };

    while let Some(event) = ctx.next_event().await {
        if let ModuleEvent::Command(command) = event {
            picker.command(command);
        }
    }
    // mochid is stopping the plugin.
    Ok(())
}

impl Picker {
    fn command(&mut self, command: ModuleCommand) {
        let action = command.action.clone();
        match action.as_str() {
            "search" => {
                let query = command.args.str("query").unwrap_or("");
                let found = search(&self.emoji, query, &self.recents, self.max_results);
                command.answer(Ok(output(&found)));
            }
            "pick" => {
                let id = command.args.str("id").unwrap_or("").to_owned();
                if !self.emoji.iter().any(|emoji| emoji.glyph == id) {
                    command.reply(Err(format!("no emoji {id}")));
                    return;
                }
                self.recents.push(&id);
                if let Some(file) = &self.file
                    && let Err(error) = self.recents.save(file)
                {
                    eprintln!("can't write {}: {error}", file.display());
                }
                command.reply(Ok(()));
            }
            other => command.reply(Err(format!("no action {other}"))),
        }
    }
}

/// Reads the table. A bad line is an error, with the line.
fn parse(table: &'static str) -> Result<Vec<Emoji>, &'static str> {
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
fn words(text: &str) -> Vec<String> {
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

/// The emoji for a query, best first. Without words, the recently picked
/// ones, then the rest in the table's order, which starts with smileys.
fn search<'a>(emoji: &'a [Emoji], query: &str, recents: &Recents, max: usize) -> Vec<&'a Emoji> {
    let query: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if query.is_empty() {
        let recent = recents
            .list
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

/// The search action's output: a JSON object per line.
fn output(found: &[&Emoji]) -> String {
    found
        .iter()
        .map(|emoji| {
            let result = Line {
                title: emoji.name,
                subtitle: emoji.group,
                glyph: emoji.glyph,
                copy: emoji.glyph,
                id: emoji.glyph,
            };
            serde_json::to_string(&result).expect("a result is plain strings")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `$XDG_STATE_HOME/mochi/emoji.json`, or `~/.local/state/mochi/emoji.json`.
fn recents_file() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| {
            let home = PathBuf::from(std::env::var_os("HOME")?);
            Some(home.join(".local/state"))
        })?;
    Some(state.join("mochi/emoji.json"))
}

impl Recents {
    fn new(cap: usize) -> Self {
        Self {
            list: Vec::new(),
            cap,
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.list.iter().any(|recent| recent == id)
    }

    /// Puts an emoji first, dropping the oldest past the cap.
    fn push(&mut self, id: &str) {
        self.list.retain(|recent| recent != id);
        self.list.insert(0, id.to_owned());
        self.list.truncate(self.cap);
    }

    /// Reads the file. A missing or broken one leaves the list empty.
    fn load(&mut self, file: &Path) {
        let list = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
            .unwrap_or_default();
        self.list = list;
        self.list.truncate(self.cap);
    }

    /// Writes the file through a temporary one, so it's never half written.
    fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temporary = file.with_extension("json.tmp");
        let text = serde_json::to_string(&self.list).map_err(std::io::Error::other)?;
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<Emoji> {
        parse(TABLE).expect("emoji.tsv parses")
    }

    fn glyphs(query: &str, recents: &Recents) -> Vec<&'static str> {
        search(&table(), query, recents, MAX_RESULTS)
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
    fn every_word_must_match() {
        assert!(find("zzzz").is_empty());
        assert!(find("cat zzzz").is_empty());
    }

    #[test]
    fn the_emoji_itself_finds_it() {
        assert_eq!(find("🐱")[0], "🐱");
    }

    #[test]
    fn results_are_capped() {
        assert_eq!(find("face").len(), MAX_RESULTS);
        assert_eq!(search(&table(), "face", &Recents::new(8), 3).len(), 3);
    }

    #[test]
    fn recents_come_first_without_words() {
        let mut recents = Recents::new(8);
        recents.push("🐱");
        recents.push("❤\u{fe0f}");
        let found = glyphs("", &recents);
        assert_eq!(found[..3], ["❤\u{fe0f}", "🐱", "😀"]);
        assert_eq!(found.len(), MAX_RESULTS);
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

    #[test]
    fn recents_are_most_recent_first_and_capped() {
        let mut recents = Recents::new(3);
        for id in ["a", "b", "c", "d", "b"] {
            recents.push(id);
        }
        assert_eq!(recents.list, ["b", "d", "c"]);
    }

    #[test]
    fn recents_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("mochi-emoji-{}", std::process::id()));
        let file = dir.join("mochi/emoji.json");
        let mut recents = Recents::new(8);
        recents.push("🐱");
        recents.push("😀");
        recents.save(&file).unwrap();
        let mut again = Recents::new(1);
        again.load(&file);
        assert_eq!(again.list, ["😀"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn results_are_json_lines() {
        let emoji = table();
        let found = search(&emoji, "cat face", &Recents::new(8), 2);
        let text = output(&found);
        let first = text.lines().next().unwrap();
        assert_eq!(
            first,
            r#"{"title":"cat face","subtitle":"animals & nature","glyph":"🐱","copy":"🐱","id":"🐱"}"#
        );
        assert_eq!(text.lines().count(), 2);
        for line in text.lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value["title"].is_string());
        }
    }

    /// settings.toml with each `# key = value` line uncommented, the way
    /// mochid reads the defaults.
    #[test]
    fn the_example_settings_are_the_defaults() {
        let example = include_str!("../settings.toml");
        let uncommented: String = example
            .lines()
            .map(|line| match line.strip_prefix("# ") {
                Some(rest) if rest.contains(" = ") => rest,
                _ => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut table: toml::Table = toml::from_str(&uncommented).unwrap();
        let section = table["module"]
            .as_table_mut()
            .unwrap()
            .remove("emoji-example");
        let settings: Settings = section.unwrap().try_into().unwrap();
        assert_eq!(settings, Settings::default());
    }
}

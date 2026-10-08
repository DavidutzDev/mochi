//! An emoji picker. `mochi ipc emoji toggle`, bound to a key, grows the
//! island into a grid of emoji with a search box and a tab per Unicode
//! group; picking one pastes it into the window you were in. The launcher
//! finds them too, after `:`.
//!
//! The table comes from Unicode's emoji-test.txt, see `data/generate.sh`.
//! The module publishes it whole, with the words the search looks at, so
//! the panel filters as you type without asking. The emoji picked last are
//! kept in `$XDG_STATE_HOME/mochi/emoji.json` and come first. Pasting and
//! copying go through the clipboard module.
//!
//! Emoji of people and hands come in a skin tone: a default one, or one
//! chosen for that emoji, see [`tones`]. The same file keeps them.

mod recents;
mod search;
mod tones;

use std::path::PathBuf;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, ContributionSpec,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::recents::Recents;
use crate::search::{Emoji as Entry, GROUPS, TABLE};
use crate::tones::{Tone, Tones};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The most results the launcher's search answers with.
const MAX_RESULTS: usize = 50;

#[derive(Debug, Default)]
pub struct Emoji;

#[derive(Debug, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    recent: usize,
    max_results: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recent: 24,
            max_results: MAX_RESULTS,
        }
    }
}

impl Module for Emoji {
    fn id(&self) -> &'static str {
        "emoji"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        // No view: the launcher shows what `search` answers.
        vec![
            ContributionSpec::new("launcher", "provider", "emoji", "", "Emoji")
                .icon("face-smile")
                .options(json!({ "prefix": ":", "search": "search", "pick": "pick" })),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let emoji = |what| ArgSpec::string("emoji", what);
        vec![
            ActionSpec::new("toggle", "Open the picker, or close it when open"),
            ActionSpec::new("open", "Open the picker"),
            ActionSpec::new("close", "Close the picker"),
            ActionSpec::new(
                "paste",
                "Paste an emoji into the window you were in; the picker sends this",
            )
            .arg(emoji("The emoji to paste")),
            ActionSpec::new("copy", "Copy an emoji; the picker sends this")
                .arg(emoji("The emoji to copy")),
            ActionSpec::new(
                "search",
                "Answer with the emoji matching some words, as launcher results",
            )
            .arg(
                ArgSpec::string("query", "Words of the name, like cat face")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new(
                "pick",
                "Remember an emoji as picked, so it comes first next time",
            )
            .arg(ArgSpec::string("id", "The emoji")),
            ActionSpec::new(
                "tone",
                "Set the skin tone emoji of people and hands show in, or one emoji's own",
            )
            .arg(ArgSpec::choice("tone", "The tone", Tone::NAMES))
            .arg(ArgSpec::string("emoji", "Only for this emoji, from then on").optional()),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let emoji =
                search::parse(TABLE).map_err(|line| format!("bad line in emoji.tsv: {line}"))?;
            let file = recents::file();
            let mut recents = Recents::new(settings.recent);
            let mut tones = Tones::default();
            if let Some(file) = &file {
                recents::load(file, &mut recents, &mut tones);
            }
            let mut picker = Picker {
                table: table_state(&emoji),
                emoji,
                recents,
                tones,
                max_results: settings.max_results.min(MAX_RESULTS),
                file,
                shown: None,
            };
            picker.publish(&ctx);

            while let Some(event) = ctx.next_event().await {
                match event {
                    ModuleEvent::Command(command) => picker.command(&ctx, command),
                    ModuleEvent::Ended { activity, .. } if picker.shown == Some(activity) => {
                        picker.shown = None;
                    }
                    _ => {}
                }
            }
            Ok(())
        })
    }
}

#[derive(Debug)]
struct Picker {
    emoji: Vec<Entry>,
    /// The table as the panel gets it, made once.
    table: Value,
    recents: Recents,
    tones: Tones,
    max_results: usize,
    /// Where the recents are kept. `None` without a home directory.
    file: Option<PathBuf>,
    shown: Option<ActivityId>,
}

impl Picker {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let result = match command.action.as_str() {
            "toggle" if self.shown.is_some() => {
                self.close(ctx);
                Ok(())
            }
            "toggle" | "open" => {
                self.open(ctx);
                Ok(())
            }
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "search" => {
                let query = command.args.str("query").unwrap_or_default();
                let found = search::search(&self.emoji, query, &self.recents, self.max_results);
                command.answer(Ok(lines(&found, &self.tones)));
                return;
            }
            "pick" => {
                let id = command.args.str("id").unwrap_or_default().to_owned();
                self.picked(ctx, &id)
            }
            "paste" | "copy" => {
                let glyph = command.args.str("emoji").unwrap_or_default().to_owned();
                self.picked(ctx, &glyph).map(|()| {
                    self.close(ctx);
                    let action = if command.action == "paste" {
                        "paste-text"
                    } else {
                        "copy-text"
                    };
                    clipboard(ctx, action, glyph);
                })
            }
            "tone" => {
                let tone = command.args.str("tone").unwrap_or_default().to_owned();
                let emoji = command.args.str("emoji").map(str::to_owned);
                self.tone(ctx, &tone, emoji.as_deref())
            }
            other => Err(format!("emoji has no action {other}")),
        };
        command.reply(result);
    }

    fn open(&mut self, ctx: &ModuleCtx) {
        ctx.close_other_panels();
        let spec = ActivitySpec::new("Picker")
            .key("picker")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            // Only the island on this monitor takes the keyboard.
            .payload(json!({ "output": ctx.compositor().state().focused_output }));
        self.shown = Some(ctx.present(spec));
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown.take() {
            ctx.withdraw(id);
        }
    }

    /// Puts an emoji first in the recents, without its tone, and saves
    /// them.
    fn picked(&mut self, ctx: &ModuleCtx, glyph: &str) -> Result<(), String> {
        let Some((emoji, _)) = tones::find(&self.emoji, glyph) else {
            return Err(format!("no emoji {glyph}"));
        };
        self.recents.push(emoji.glyph);
        self.save();
        self.publish(ctx);
        Ok(())
    }

    /// Sets the default tone, or with an emoji, that emoji's own.
    fn tone(&mut self, ctx: &ModuleCtx, name: &str, glyph: Option<&str>) -> Result<(), String> {
        let tone = Tone::parse(name).ok_or_else(|| format!("no skin tone {name}"))?;
        match glyph {
            None => self.tones.default = tone,
            Some(glyph) => {
                let Some((emoji, _)) = tones::find(&self.emoji, glyph) else {
                    return Err(format!("no emoji {glyph}"));
                };
                if emoji.tones.is_none() {
                    return Err(format!("{glyph} has no skin tones"));
                }
                self.tones.chosen.insert(emoji.glyph.to_owned(), tone);
            }
        }
        self.save();
        self.publish(ctx);
        Ok(())
    }

    fn save(&self) {
        if let Some(file) = &self.file
            && let Err(error) = recents::save(file, &self.recents, &self.tones)
        {
            tracing::warn!(%error, file = %file.display(), "could not save the recent emoji");
        }
    }

    fn publish(&self, ctx: &ModuleCtx) {
        let mut state = self.table.clone();
        state["recent"] = json!(self.recents.list());
        state["tone"] = json!(self.tones.default.index());
        let chosen: serde_json::Map<String, Value> = self
            .tones
            .chosen
            .iter()
            .map(|(glyph, tone)| (glyph.clone(), json!(tone.index())))
            .collect();
        state["tones"] = Value::Object(chosen);
        ctx.publish_state(state);
    }
}

/// The table for the panel: the groups, with where each starts in the
/// list, and every emoji as `[glyph, name, group, name words, subgroup
/// words, tones]`, words lowercase and joined by spaces, tones the five
/// toned emoji or null.
fn table_state(emoji: &[Entry]) -> Value {
    let groups: Vec<Value> = GROUPS
        .iter()
        .map(|(group, title, glyph)| {
            let start = emoji.iter().position(|emoji| emoji.group == *group);
            let count = emoji.iter().filter(|emoji| emoji.group == *group).count();
            json!({
                "title": title,
                "glyph": glyph,
                "words": search::words(group).join(" "),
                "start": start.unwrap_or(0),
                "count": count,
            })
        })
        .collect();
    let list: Vec<Value> = emoji
        .iter()
        .map(|emoji| {
            let group = GROUPS
                .iter()
                .position(|(group, _, _)| *group == emoji.group);
            json!([
                emoji.glyph,
                emoji.name,
                group,
                emoji.name_words.join(" "),
                emoji.subgroup_words.join(" "),
                emoji.tones,
            ])
        })
        .collect();
    json!({ "groups": groups, "emoji": list })
}

/// A line of the search action's answer, as the launcher reads it.
#[derive(Debug, Serialize)]
struct Line<'a> {
    title: &'a str,
    subtitle: &'a str,
    glyph: &'a str,
    /// Enter pastes it.
    #[serde(rename = "type")]
    paste: &'a str,
    /// Shift+Enter copies it.
    alt: Alt<'a>,
    id: &'a str,
}

#[derive(Debug, Serialize)]
struct Alt<'a> {
    copy: &'a str,
}

/// The search action's answer: a JSON object per line. Each emoji is in
/// its skin tone; the id, which `pick` gets back, is without it.
fn lines(found: &[&Entry], tones: &Tones) -> String {
    found
        .iter()
        .map(|emoji| {
            let glyph = tones.apply(emoji);
            let line = Line {
                title: emoji.name,
                subtitle: emoji.group,
                glyph,
                paste: glyph,
                alt: Alt { copy: glyph },
                id: emoji.glyph,
            };
            serde_json::to_string(&line).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Copies or pastes through the clipboard module, which owns the
/// selection. Copying works without it when `wl-copy` is there.
fn clipboard(ctx: &ModuleCtx, action: &'static str, text: String) {
    let call = ctx.call("clipboard", action, &[&text]);
    tokio::spawn(async move {
        match call.await {
            Ok(()) => {}
            Err(CallError::NotEnabled(_)) if action == "copy-text" => {
                let copied = mochi_core::process::spawn_detached(
                    &["wl-copy".into(), "--".into(), text],
                    None,
                );
                if let Err(error) = copied {
                    tracing::warn!(%error, "can't copy: enable the clipboard module or install wl-copy");
                }
            }
            Err(error) => tracing::warn!(%error, action, "the clipboard module couldn't do it"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<Entry> {
        search::parse(TABLE).unwrap()
    }

    #[test]
    fn a_result_is_one_json_line() {
        let emoji = table();
        let found = search::search(&emoji, "cat face", &Recents::new(8), 2);
        let text = lines(&found, &Tones::default());
        assert_eq!(
            text.lines().next().unwrap(),
            r#"{"title":"cat face","subtitle":"animals & nature","glyph":"🐱","type":"🐱","alt":{"copy":"🐱"},"id":"🐱"}"#
        );
        assert_eq!(text.lines().count(), 2);
        assert_eq!(lines(&[], &Tones::default()), "");
    }

    #[test]
    fn results_come_in_their_skin_tone() {
        let emoji = table();
        let mut tones = Tones {
            default: Tone::Medium,
            ..Tones::default()
        };
        tones.chosen.insert("👎".into(), Tone::None);
        let found = search::search(&emoji, "thumbs", &Recents::new(8), 2);
        let text = lines(&found, &tones);
        let mut text = text.lines();
        assert_eq!(
            text.next().unwrap(),
            r#"{"title":"thumbs up","subtitle":"people & body","glyph":"👍🏽","type":"👍🏽","alt":{"copy":"👍🏽"},"id":"👍"}"#
        );
        assert!(text.next().unwrap().contains(r#""glyph":"👎","#));
    }

    #[test]
    fn the_published_table_has_every_emoji_in_its_group() {
        let emoji = table();
        let state = table_state(&emoji);
        let list = state["emoji"].as_array().unwrap();
        assert_eq!(list.len(), emoji.len());
        assert_eq!(
            list[0],
            json!([
                "😀",
                "grinning face",
                0,
                "grinning face",
                "face smiling",
                null
            ])
        );
        let thumbs = list.iter().find(|entry| entry[0] == "👍").unwrap();
        assert_eq!(thumbs[5], json!(["👍🏻", "👍🏼", "👍🏽", "👍🏾", "👍🏿"]));
        let groups = state["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 9);
        let mut next = 0;
        for (index, group) in groups.iter().enumerate() {
            let start = group["start"].as_u64().unwrap() as usize;
            let count = group["count"].as_u64().unwrap() as usize;
            assert_eq!(
                start, next,
                "{} starts where the last ended",
                group["title"]
            );
            assert!(count > 0);
            assert!(
                list[start..start + count]
                    .iter()
                    .all(|entry| entry[2] == index)
            );
            next = start + count;
        }
        assert_eq!(next, list.len());
        assert_eq!(groups[0]["words"], "smileys emotion");
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "emoji",
            include_str!("../settings.toml"),
        );
    }
}

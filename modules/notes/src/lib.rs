//! To-do lists and notes, as desktop widgets. Each placed widget, an
//! instance, has its own list or text, kept in
//! `$XDG_STATE_HOME/mochi/notes.json` and published by instance id, so
//! every view shows its own. Views change them with the actions below,
//! typed into on the desktop.

mod tour;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ArgSpec, Assets, BoxFuture, ContributionSpec, Module, ModuleCommand, ModuleCtx,
    ModuleError, ModuleEvent,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Notes;

impl Module for Notes {
    fn id(&self) -> &'static str {
        "notes"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("widgets", "widget", "todo", "Todo", "To-do")
                .icon("check")
                .options(json!({
                    "size": [16, 14],
                    "min": [10, 6],
                    "max": [40, 50],
                    "forget": "forget",
                    "settings": [
                        {
                            "name": "title",
                            "default": "To-do",
                            "description": "The heading over the list",
                        },
                        {
                            "name": "done",
                            "kind": "choice",
                            "choices": ["show", "hide"],
                            "default": "show",
                            "description": "Whether done items stay in the list",
                        },
                    ],
                })),
            ContributionSpec::new("widgets", "widget", "note", "Note", "Note")
                .icon("edit")
                .options(json!({
                    "size": [16, 12],
                    "min": [8, 5],
                    "max": [60, 50],
                    "forget": "forget",
                    "settings": [
                        {
                            "name": "title",
                            "default": "Note",
                            "description": "The heading over the text; empty for none",
                        },
                    ],
                })),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let instance = || ArgSpec::string("instance", "The widget's id, like w3");
        let index = || ArgSpec::int("index", "The item's place in the list, from 0");
        vec![
            ActionSpec::new("write", "Replace a note's text")
                .arg(instance())
                .arg(ArgSpec::string("text", "The text").optional().rest()),
            ActionSpec::new("add", "Add an item to a to-do list")
                .arg(instance())
                .arg(ArgSpec::string("text", "The item").rest()),
            ActionSpec::new("toggle", "Mark an item done, or not done")
                .arg(instance())
                .arg(index()),
            ActionSpec::new("delete", "Remove an item")
                .arg(instance())
                .arg(index()),
            ActionSpec::new("clear", "Remove the items that are done").arg(instance()),
            ActionSpec::new(
                "forget",
                "Drop everything a widget kept; the widgets module sends this when one is removed",
            )
            .arg(instance()),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let path = state_file();
            let mut book = path.as_deref().map(Book::load).unwrap_or_default();
            ctx.publish_state(book.state());
            while let Some(event) = ctx.next_event().await {
                if let ModuleEvent::Command(command) = event {
                    let result = book.command(&command);
                    if result.is_ok() {
                        if let Some(path) = &path
                            && let Err(error) = book.save(path)
                        {
                            tracing::warn!(%error, "could not save the notes");
                        }
                        ctx.publish_state(book.state());
                    }
                    command.reply(result);
                }
            }
            Ok(())
        })
    }
}

/// `$XDG_STATE_HOME/mochi/notes.json`, falling back to `~/.local/state`.
fn state_file() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/state")))?;
    Some(state.join("mochi").join("notes.json"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Item {
    text: String,
    #[serde(default)]
    done: bool,
}

/// One widget's content: a to-do list's items, or a note's text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Page {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    items: Vec<Item>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text: String,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Book {
    pages: BTreeMap<String, Page>,
}

impl Book {
    /// The saved notes, or none when the file is missing or broken.
    fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temporary, path)
    }

    fn state(&self) -> serde_json::Value {
        json!({ "pages": self.pages })
    }

    fn command(&mut self, command: &ModuleCommand) -> Result<(), String> {
        let args = &command.args;
        let instance = args.str("instance").unwrap_or_default().to_owned();
        if instance.is_empty() {
            return Err("no widget named".into());
        }
        let text = args.str("text").unwrap_or_default().trim_end().to_owned();
        match command.action.as_str() {
            "write" => {
                self.pages.entry(instance).or_default().text = text;
            }
            "add" => {
                if text.trim().is_empty() {
                    return Err("an item needs some text".into());
                }
                self.pages
                    .entry(instance)
                    .or_default()
                    .items
                    .push(Item { text, done: false });
            }
            "toggle" => self.item(&instance, args.int("index"))?.done ^= true,
            "delete" => {
                let index = self.index(&instance, args.int("index"))?;
                self.pages.entry(instance).or_default().items.remove(index);
            }
            "clear" => self
                .pages
                .entry(instance)
                .or_default()
                .items
                .retain(|item| !item.done),
            "forget" => {
                self.pages.remove(&instance);
            }
            other => return Err(format!("notes has no action {other}")),
        }
        // Nothing left: nothing to keep.
        self.pages
            .retain(|_, page| !page.items.is_empty() || !page.text.is_empty());
        Ok(())
    }

    fn index(&self, instance: &str, index: Option<i64>) -> Result<usize, String> {
        let count = self.pages.get(instance).map_or(0, |page| page.items.len());
        usize::try_from(index.unwrap_or(-1))
            .ok()
            .filter(|index| *index < count)
            .ok_or_else(|| format!("{instance} has no item {}", index.unwrap_or(-1)))
    }

    fn item(&mut self, instance: &str, index: Option<i64>) -> Result<&mut Item, String> {
        let index = self.index(instance, index)?;
        Ok(&mut self.pages.get_mut(instance).expect("checked").items[index])
    }
}

#[cfg(test)]
mod tests {
    use mochi_core::{ArgValue, Args};

    use super::*;

    fn run(book: &mut Book, action: &str, args: &[(&str, ArgValue)]) -> Result<(), String> {
        let args: Args = args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect();
        let (command, _reply) = ModuleCommand::new(action.into(), args);
        book.command(&command)
    }

    fn text(value: &str) -> ArgValue {
        ArgValue::String(value.into())
    }

    #[test]
    fn keeps_lists_and_notes_per_widget() {
        let mut book = Book::default();
        let w1 = ("instance", text("w1"));
        run(&mut book, "add", &[w1.clone(), ("text", text("Milk"))]).unwrap();
        run(&mut book, "add", &[w1.clone(), ("text", text("Taxes"))]).unwrap();
        run(
            &mut book,
            "toggle",
            &[w1.clone(), ("index", ArgValue::Int(0))],
        )
        .unwrap();
        run(
            &mut book,
            "write",
            &[
                ("instance", text("w2")),
                ("text", text("line one\nline two\n")),
            ],
        )
        .unwrap();
        assert!(book.pages["w1"].items[0].done);
        assert_eq!(book.pages["w2"].text, "line one\nline two");

        assert!(
            run(
                &mut book,
                "toggle",
                &[w1.clone(), ("index", ArgValue::Int(5))]
            )
            .is_err()
        );
        assert!(run(&mut book, "add", &[w1.clone(), ("text", text("  "))]).is_err());

        run(&mut book, "clear", std::slice::from_ref(&w1)).unwrap();
        assert_eq!(book.pages["w1"].items.len(), 1);
        run(
            &mut book,
            "delete",
            &[w1.clone(), ("index", ArgValue::Int(0))],
        )
        .unwrap();
        // An empty page is dropped.
        assert!(!book.pages.contains_key("w1"));
        run(&mut book, "forget", &[("instance", text("w2"))]).unwrap();
        assert!(book.pages.is_empty());
    }

    #[test]
    fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("mochi-notes-{}", std::process::id()));
        let path = dir.join("notes.json");
        let mut book = Book::default();
        run(
            &mut book,
            "add",
            &[("instance", text("w1")), ("text", text("Milk"))],
        )
        .unwrap();
        book.save(&path).unwrap();
        assert_eq!(Book::load(&path), book);
        std::fs::write(&path, "broken").unwrap();
        assert_eq!(Book::load(&path), Book::default());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

//! The settings panel: every option of the theme, the island, the bubbles,
//! each module and each plugin, changed live. The daemon keeps the changes
//! in `changes.toml` over `config.toml` and `theme.toml`; Copy gives them
//! as the `settings` and `theme` of home-manager's `programs.mochi`, or as
//! TOML, to keep them in the files.
//!
//! `mochi ipc settings open` opens it, at a section with
//! `mochi ipc settings open colors`. The launcher finds sections and
//! options by name.

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, ContributionSpec,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority, SettingsOp,
};
use serde_json::{Value, json};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The most results the launcher lists.
const RESULTS: usize = 8;

#[derive(Debug, Default)]
pub struct Settings;

impl Module for Settings {
    fn id(&self) -> &'static str {
        "settings"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let path = || ArgSpec::string("path", "From the file, like theme.colors.accent");
        vec![
            ActionSpec::new("toggle", "Open the settings, or close them when open"),
            ActionSpec::new("open", "Open the settings").arg(
                ArgSpec::string(
                    "section",
                    "A section, like colors, island or a module's id, or an option's path",
                )
                .optional(),
            ),
            ActionSpec::new("close", "Close the settings"),
            ActionSpec::new("set", "Change an option").arg(path()).arg(
                ArgSpec::string("value", "Its new value as JSON, like 2000 or \"#30d158\"")
                    .rest(),
            ),
            ActionSpec::new(
                "reset",
                "Put an option, or each option of a section, back to its default",
            )
            .arg(path()),
            ActionSpec::new(
                "discard",
                "Drop every change made here, back to what the files say",
            ),
            ActionSpec::new("text", "Print a section as TOML").arg(path()),
            ActionSpec::new("edit", "Replace a section with TOML")
                .arg(path())
                .arg(ArgSpec::string("text", "The section's TOML").rest()),
            ActionSpec::new(
                "export",
                "Print every option that isn't at its default, for home-manager or the files",
            )
            .arg(ArgSpec::choice("format", "nix or toml", ["nix", "toml"]).optional()),
            ActionSpec::new("copy", "Copy what export prints")
                .arg(ArgSpec::choice("format", "nix or toml", ["nix", "toml"]).optional()),
            ActionSpec::new(
                "search",
                "List the sections and options a query names, as JSON lines; the launcher sends this",
            )
            .arg(ArgSpec::string("query", "What to look for").optional().rest()),
            ActionSpec::new("pick-result", "Open what the launcher listed")
                .arg(ArgSpec::string("id", "A section or an option's path")),
        ]
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("launcher", "provider", "settings", "", "Settings").options(
                json!({
                    "search": "search",
                    "pick": "pick-result",
                }),
            ),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let mut panel = Panel::default();
            match ctx.settings_op(SettingsOp::Snapshot).await {
                Ok(snapshot) => panel.snapshot = snapshot,
                Err(error) => tracing::warn!(%error, "no settings to show"),
            }
            panel.publish(&ctx);
            while let Some(event) = ctx.next_event().await {
                match event {
                    ModuleEvent::Command(command) => panel.command(&ctx, command).await,
                    ModuleEvent::Settings(snapshot) => {
                        panel.snapshot = snapshot;
                        panel.publish(&ctx);
                    }
                    ModuleEvent::Ended { activity, .. } if panel.shown == Some(activity) => {
                        panel.shown = None;
                        panel.editor = None;
                        panel.error = None;
                        panel.publish(&ctx);
                    }
                    _ => {}
                }
            }
            Ok(())
        })
    }
}

#[derive(Debug, Default)]
struct Panel {
    snapshot: Value,
    shown: Option<ActivityId>,
    /// The last change the daemon refused: `{path, message}`.
    error: Option<Value>,
    /// The section open in the TOML editor: `{path, text}`.
    editor: Option<Value>,
}

impl Panel {
    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let arg = |name: &str| args.str(name).unwrap_or_default().to_owned();
        let result = match command.action.as_str() {
            "toggle" if self.shown.is_some() => {
                self.close(ctx);
                Ok(())
            }
            "toggle" | "open" => {
                self.open(ctx, args.str("section"));
                Ok(())
            }
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "set" => {
                let path = arg("path");
                match serde_json::from_str::<Value>(&arg("value")) {
                    Ok(value) => {
                        self.change(
                            ctx,
                            &path,
                            SettingsOp::Set {
                                path: path.clone(),
                                value,
                            },
                        )
                        .await
                    }
                    Err(error) => Err(format!("the value isn't JSON: {error}")),
                }
            }
            "reset" => {
                let path = arg("path");
                self.change(ctx, &path, SettingsOp::Reset { path: path.clone() })
                    .await
            }
            "discard" => self.change(ctx, "", SettingsOp::Discard).await,
            "edit" => {
                let path = arg("path");
                let op = SettingsOp::Edit {
                    path: path.clone(),
                    text: arg("text"),
                };
                let result = self.change(ctx, &path, op).await;
                if result.is_ok() {
                    self.editor = None;
                    self.publish(ctx);
                }
                result
            }
            "text" => {
                let path = arg("path");
                let text = ctx
                    .settings_op(SettingsOp::Text { path: path.clone() })
                    .await;
                match text {
                    Ok(Value::String(text)) => {
                        self.editor = Some(json!({ "path": path, "text": text }));
                        self.error = None;
                        self.publish(ctx);
                        command.answer(Ok(text));
                        return;
                    }
                    Ok(_) => Err("no text".to_owned()),
                    Err(error) => Err(error),
                }
            }
            "export" | "copy" => {
                let format = args.str("format").unwrap_or("nix").to_owned();
                match ctx.settings_op(SettingsOp::Export { format }).await {
                    Ok(Value::String(text)) if command.action == "copy" => {
                        copy(ctx, text);
                        Ok(())
                    }
                    Ok(Value::String(text)) => {
                        command.answer(Ok(text));
                        return;
                    }
                    Ok(_) => Err("nothing to export".to_owned()),
                    Err(error) => Err(error),
                }
            }
            "search" => {
                let lines: Vec<String> = search(&self.snapshot, args.str("query").unwrap_or(""))
                    .iter()
                    .map(Value::to_string)
                    .collect();
                command.answer(Ok(lines.join("\n")));
                return;
            }
            "pick-result" => {
                self.open(ctx, args.str("id"));
                Ok(())
            }
            other => Err(format!("settings has no action {other}")),
        };
        command.reply(result);
    }

    /// Runs a change. A refused one shows its message under the option
    /// until the next change.
    async fn change(&mut self, ctx: &ModuleCtx, path: &str, op: SettingsOp) -> Result<(), String> {
        let result = ctx.settings_op(op).await.map(drop);
        self.error = result
            .as_ref()
            .err()
            .map(|message| json!({ "path": path, "message": message }));
        // The new snapshot comes as an event; an error only changes this.
        if result.is_err() {
            self.publish(ctx);
        }
        result
    }

    fn open(&mut self, ctx: &ModuleCtx, target: Option<&str>) {
        let (section, option) = locate(&self.snapshot, target.unwrap_or(""));
        if let Some(id) = self.shown {
            // Already open: only go there.
            ctx.update(id, payload(ctx, &section, &option));
            return;
        }
        ctx.close_other_panels();
        let spec = ActivitySpec::new("Panel")
            .key("settings")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(payload(ctx, &section, &option));
        self.shown = Some(ctx.present(spec));
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown.take() {
            ctx.withdraw(id);
        }
    }

    fn publish(&self, ctx: &ModuleCtx) {
        let mut state = self.snapshot.clone();
        if !state.is_object() {
            state = json!({});
        }
        state["error"] = self.error.clone().unwrap_or(Value::Null);
        state["editor"] = self.editor.clone().unwrap_or(Value::Null);
        ctx.publish_state(state);
    }
}

fn payload(ctx: &ModuleCtx, section: &str, option: &str) -> Value {
    json!({
        // Only the island on this monitor takes the keyboard.
        "output": ctx.compositor().state().focused_output,
        "section": section,
        "option": option,
    })
}

fn sections(snapshot: &Value) -> &[Value] {
    snapshot["sections"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// The section and the option `target` names: a section's id or path, or
/// an option's path. An empty or unknown one opens the first section.
fn locate(snapshot: &Value, target: &str) -> (String, String) {
    for section in sections(snapshot) {
        let id = section["id"].as_str().unwrap_or_default();
        if target == id || section["path"] == target {
            return (id.to_owned(), String::new());
        }
        let fields = section["fields"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        if fields.iter().any(|field| field["path"] == target) {
            return (id.to_owned(), target.to_owned());
        }
    }
    (String::new(), String::new())
}

/// The launcher's results: sections whose title matches, then options
/// whose title or description does. "settings" alone lists the panel.
fn search(snapshot: &Value, query: &str) -> Vec<Value> {
    let query = query.trim().to_lowercase();
    if query.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::new();
    if "settings".starts_with(&query) || "preferences".starts_with(&query) {
        out.push(json!({
            "title": "Settings",
            "subtitle": "Mochi's options, changed live",
            "icon": "settings",
            "id": "",
        }));
    }
    let matches = |text: &str| text.to_lowercase().contains(&query);
    for section in sections(snapshot) {
        let title = section["title"].as_str().unwrap_or_default();
        if matches(title) {
            out.push(json!({
                "title": title,
                "subtitle": "Settings",
                "icon": section["icon"],
                "id": section["id"],
            }));
        }
    }
    for section in sections(snapshot) {
        let heading = section["title"].as_str().unwrap_or_default();
        for field in section["fields"].as_array().into_iter().flatten() {
            let title = field["title"].as_str().unwrap_or_default();
            if field["kind"] == "group" || !matches(title) {
                continue;
            }
            out.push(json!({
                "title": title,
                "subtitle": format!("Settings › {heading}"),
                "icon": section["icon"],
                "id": field["path"],
            }));
        }
    }
    out.truncate(RESULTS);
    out
}

fn copy(ctx: &ModuleCtx, text: String) {
    let call = ctx.call("clipboard", "copy-text", &[&text]);
    tokio::spawn(async move {
        match call.await {
            Ok(()) => {}
            Err(CallError::NotEnabled(_)) => {
                let copied = mochi_core::process::spawn_detached(
                    &["wl-copy".into(), "--".into(), text],
                    None,
                );
                if let Err(error) = copied {
                    tracing::warn!(%error, "can't copy: enable the clipboard module or install wl-copy");
                }
            }
            Err(error) => tracing::warn!(%error, "the clipboard module couldn't copy it"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Value {
        json!({
            "sections": [
                {
                    "id": "colors",
                    "path": "theme.colors",
                    "title": "Colors",
                    "icon": "palette",
                    "fields": [
                        { "path": "theme.colors.accent", "title": "Accent", "kind": "color" },
                    ],
                },
                {
                    "id": "osd",
                    "path": "config.module.osd",
                    "title": "OSD",
                    "icon": "tune",
                    "fields": [
                        { "path": "config.module.osd.timeout_ms", "title": "Timeout (ms)", "kind": "int" },
                    ],
                },
            ],
        })
    }

    #[test]
    fn targets_name_a_section_or_an_option() {
        let snapshot = snapshot();
        assert_eq!(locate(&snapshot, "osd"), ("osd".into(), String::new()));
        assert_eq!(
            locate(&snapshot, "theme.colors"),
            ("colors".into(), String::new())
        );
        assert_eq!(
            locate(&snapshot, "theme.colors.accent"),
            ("colors".into(), "theme.colors.accent".into())
        );
        assert_eq!(locate(&snapshot, "nope"), (String::new(), String::new()));
    }

    #[test]
    fn the_launcher_finds_sections_and_options() {
        let snapshot = snapshot();
        let titles = |query| {
            search(&snapshot, query)
                .iter()
                .map(|result| result["title"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(titles("sett"), ["Settings"]);
        assert_eq!(titles("acc"), ["Accent"]);
        assert_eq!(titles("osd"), ["OSD"]);
        assert_eq!(titles("timeout"), ["Timeout (ms)"]);
        assert!(titles("a").is_empty());
    }
}

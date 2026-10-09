//! The settings panel: every option of the theme, the island, the bubbles,
//! each module and each plugin, changed live. The daemon keeps the changes
//! in `changes.toml` over `config.toml` and `theme.toml`; Copy gives them
//! as the `settings` and `theme` of home-manager's `programs.mochi`, or as
//! TOML, to keep them in the files.
//!
//! `mochi ipc settings open` opens it, at a section with
//! `mochi ipc settings open colors`. The launcher finds sections and
//! options by name.

mod about;
mod bento;
mod tour;

use std::path::Path;

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
        let source = || {
            ArgSpec::string(
                "source",
                "An id in the registry, a directory, or a repository",
            )
        };
        vec![
            ActionSpec::new("toggle", "Open the settings, or close them when open"),
            ActionSpec::new("open", "Open the settings").arg(
                ArgSpec::string(
                    "section",
                    "A section, like colors, island or a module's id, or an option's path",
                )
                .optional()
                .source("settings-section"),
            ),
            ActionSpec::new("close", "Close the settings"),
            ActionSpec::new("set", "Change an option").arg(path()).arg(
                ArgSpec::string("value", "Its new value as JSON, like 2000 or \"#30d158\"")
                    .rest(),
            ),
            ActionSpec::new(
                "reset",
                "Undo the change to an option, or to each option of a section, back to what the files say",
            )
            .arg(path()),
            ActionSpec::new(
                "discard",
                "Drop every change made here, back to what the files say",
            ),
            ActionSpec::new(
                "preview",
                "Try an option without keeping it; a reload forgets it",
            )
            .arg(path())
            .arg(ArgSpec::string("value", "Its value as JSON").rest()),
            ActionSpec::new(
                "try",
                "Try whole tables of settings without keeping them, like a bento's",
            )
            .arg(
                ArgSpec::string("changes", "JSON like {\"config\": {...}, \"theme\": {...}}")
                    .rest(),
            ),
            ActionSpec::new("keep", "Keep what's being tried, as changes"),
            ActionSpec::new("drop", "Stop trying, back to the changes"),
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
            ActionSpec::new(
                "bento-catalog",
                "Read Bento's registry and what it installed, for the Bento pages",
            )
            .arg(ArgSpec::choice("refresh", "Download the registry again", ["refresh"]).optional()),
            ActionSpec::new("bento-plan", "Say what installing something would do")
                .arg(source()),
            ActionSpec::new("bento-show", "Open the Bento page on something to install")
                .arg(source()),
            ActionSpec::new("bento-add", "Install a bento, a theme or a plugin")
                .arg(source())
                .arg(ArgSpec::string("at", "The commit its plan showed").optional()),
            ActionSpec::new("bento-remove", "Take out what Bento installed")
                .arg(ArgSpec::string("id", "Its id")),
            ActionSpec::new("bento-update", "Move what Bento installed to newer releases")
                .arg(ArgSpec::string("id", "Only this").optional()),
            ActionSpec::new("bento-try", "Try a theme or a bento's look").arg(source()),
            ActionSpec::new(
                "bento-use",
                "Switch to a bento, back to your own setup with mine, or put a theme on",
            )
            .arg(ArgSpec::string("name", "A bento's or a theme's id, or mine")),
            ActionSpec::new("bento-share", "Make a bento of this setup")
                .arg(ArgSpec::string("dir", "The directory to write"))
                .arg(
                    ArgSpec::string(
                        "what",
                        "theme for a theme, or the parts by comma: theme, shell, modules, settings, module:<id>, widgets, wallpaper",
                    )
                    .optional(),
                )
                .arg(ArgSpec::string("name", "What it's called").optional().rest()),
            ActionSpec::new("bento-parts", "Say what this setup has to share, for the Share page"),
            ActionSpec::new("bento-forget", "Close what the Bento pages show about a plan or a share"),
            ActionSpec::new("about", "Read what the About page shows"),
            ActionSpec::new("about-copy", "Copy the About page as text, for a bug report"),
            ActionSpec::new("open-link", "Open one of Mochi's pages in the browser").arg(
                ArgSpec::string("page", "repository, documentation or issue"),
            ),
        ]
    }

    fn needs(&self, _settings: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        vec![
            mochi_core::Need::new(
                "wl-copy",
                "Copy as Nix or TOML, while the clipboard module is off",
            ),
            mochi_core::Need::new("xdg-open", "The links on the About page"),
        ]
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("launcher", "provider", "settings", "", "Settings").options(
                json!({
                    "search": "search",
                    "pick": "pick-result",
                }),
            ),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let mut panel = Panel::default();
            match ctx.settings_op(SettingsOp::Snapshot).await {
                Ok(snapshot) => panel.snapshot = snapshot,
                Err(error) => tracing::warn!(%error, "no settings to show"),
            }
            let (done, mut answers) = tokio::sync::mpsc::unbounded_channel();
            panel.done = Some(done.clone());
            let (read, mut abouts) = tokio::sync::mpsc::unbounded_channel();
            panel.read = Some(read);
            panel.publish(&ctx);
            loop {
                let event = tokio::select! {
                    event = ctx.next_event() => match event {
                        Some(event) => event,
                        None => break,
                    },
                    Some(answer) = answers.recv() => {
                        panel.bento.finished(&ctx, &done, answer);
                        panel.publish(&ctx);
                        continue;
                    }
                    Some(about) = abouts.recv() => {
                        panel.about = Some(about);
                        panel.reading = false;
                        panel.publish(&ctx);
                        continue;
                    }
                };
                match event {
                    ModuleEvent::Command(command) => panel.command(&ctx, command).await,
                    // A snapshot is large, and every view parses it, so it
                    // goes out only while the panel shows; opening it sends
                    // the latest.
                    ModuleEvent::Settings(snapshot) => {
                        panel.snapshot = snapshot;
                        if panel.shown.is_some() {
                            panel.publish(&ctx);
                        } else {
                            panel.stale = true;
                        }
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
    /// The published state is older than the snapshot.
    stale: bool,
    bento: bento::Bento,
    /// Where Bento's commands send their answers.
    done: Option<tokio::sync::mpsc::UnboundedSender<bento::Done>>,
    /// What the About page shows, once read.
    about: Option<Value>,
    /// The About page is being read.
    reading: bool,
    /// Where reading the About page sends what it found.
    read: Option<tokio::sync::mpsc::UnboundedSender<Value>>,
    /// The system's time zones, as the `timezone` source's menu lists
    /// them: `{value, label, detail}`.
    zones: Value,
    /// When the zones were read, for their offsets.
    zones_read: Option<std::time::Instant>,
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
            "preview" => {
                let path = arg("path");
                match serde_json::from_str::<Value>(&arg("value")) {
                    Ok(value) => {
                        let op = SettingsOp::Preview {
                            values: vec![(path.clone(), value)],
                            replace: false,
                        };
                        self.change(ctx, &path, op).await
                    }
                    Err(error) => Err(format!("the value isn't JSON: {error}")),
                }
            }
            "try" => match tried(&arg("changes")) {
                Ok(changes) => self.change(ctx, "", SettingsOp::Try(changes)).await,
                Err(error) => Err(error),
            },
            "keep" => self.change(ctx, "", SettingsOp::Keep).await,
            "drop" => self.change(ctx, "", SettingsOp::Drop).await,
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
            action if action.starts_with("bento-") => {
                let text = |name: &str| args.str(name).unwrap_or_default().to_owned();
                let words: Vec<String> = match action {
                    "bento-catalog" => vec![text("refresh")],
                    "bento-add" => vec![text("source"), text("at")],
                    "bento-remove" | "bento-update" => vec![text("id")],
                    "bento-use" => vec![text("name")],
                    "bento-share" => vec![text("dir"), text("what"), text("name")],
                    "bento-parts" => Vec::new(),
                    _ => vec![text("source")],
                };
                // From a link: the Bento page, on what it names.
                let action = if action == "bento-show" {
                    self.open(ctx, Some("bento"));
                    "bento-plan"
                } else {
                    action
                };
                let result = match &self.done {
                    Some(done) => self.bento.command(ctx, done, action, &words),
                    None => Err("the settings aren't ready".to_owned()),
                };
                self.publish(ctx);
                result
            }
            "about" => {
                self.read_about(ctx);
                self.publish(ctx);
                Ok(())
            }
            "about-copy" => match &self.about {
                Some(about) => {
                    copy(ctx, about::text(about));
                    Ok(())
                }
                None => Err("the About page isn't read yet".to_owned()),
            },
            "open-link" => match about::link(&arg("page")) {
                Some(url) => mochi_core::process::spawn_detached(
                    &mochi_core::process::in_app_scope(&["xdg-open".into(), url.into()]),
                    None,
                ),
                None => Err(format!(
                    "no page {:?}: repository, documentation or issue",
                    arg("page")
                )),
            },
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
        // The time zones' offsets move with daylight saving.
        let zones_old = self
            .zones_read
            .is_none_or(|read| read.elapsed() > mochi_core::zones::REFRESH);
        if zones_old {
            self.read_zones();
        }
        if self.stale || zones_old {
            self.publish(ctx);
        }
        let spec = ActivitySpec::new("Panel")
            .key("settings")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(payload(ctx, &section, &option));
        self.shown = Some(ctx.present(spec));
    }

    /// Reads the About page again, off the module's loop: asking the
    /// daemon its status waits on it.
    fn read_about(&mut self, ctx: &ModuleCtx) {
        let Some(read) = self.read.clone() else {
            return;
        };
        self.reading = true;
        let socket = ctx.socket().map(Path::to_path_buf);
        let config = ctx.config_file().map(Path::to_path_buf);
        tokio::spawn(async move {
            let _ = read.send(about::gather(socket, config).await);
        });
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown.take() {
            ctx.withdraw(id);
        }
    }

    fn publish(&mut self, ctx: &ModuleCtx) {
        self.stale = false;
        let mut state = self.snapshot.clone();
        if !state.is_object() {
            state = json!({});
        }
        state["error"] = self.error.clone().unwrap_or(Value::Null);
        state["editor"] = self.editor.clone().unwrap_or(Value::Null);
        state["bento"] = self.bento.state();
        state["about"] = self.about.clone().unwrap_or(Value::Null);
        state["about_reading"] = Value::Bool(self.reading);
        state["timezones"] = self.zones.clone();
        ctx.publish_state(state);
    }

    /// Reads the system's time zones, each with its offset now. A few
    /// hundred small files, read in a few milliseconds.
    fn read_zones(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs() as i64);
        let zones = mochi_core::zones::system(now);
        tracing::debug!(zones = zones.len(), "read the time zones");
        self.zones = zones.iter().map(mochi_core::zones::Zone::choice).collect();
        self.zones_read = Some(std::time::Instant::now());
    }
}

/// `try`'s JSON: a `config` and a `theme` table, either left out.
fn tried(text: &str) -> Result<mochi_core::changes::Changes, String> {
    let json: Value =
        serde_json::from_str(text).map_err(|error| format!("the changes aren't JSON: {error}"))?;
    let table = |name: &str| -> Result<mochi_core::toml::Table, String> {
        match json.get(name) {
            None | Some(Value::Null) => Ok(mochi_core::toml::Table::new()),
            Some(value) => mochi_core::toml::Table::try_from(value.clone())
                .map_err(|error| format!("`{name}` isn't a table: {error}")),
        }
    };
    if let Some(other) = json
        .as_object()
        .ok_or("the changes must be an object")?
        .keys()
        .find(|key| !["config", "theme"].contains(&key.as_str()))
    {
        return Err(format!("unknown part `{other}`: only `config` and `theme`"));
    }
    Ok(mochi_core::changes::Changes {
        config: table("config")?,
        theme: table("theme")?,
    })
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

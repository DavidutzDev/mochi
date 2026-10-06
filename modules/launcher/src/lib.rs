//! App launcher. `mochi ipc launcher toggle`, bound to a key, grows the
//! island into a search box: your apps, most used first, fuzzy search as
//! you type, and app actions like "New Private Window" when you search for
//! them. Enter or a click picks a result; Escape or a click elsewhere
//! closes it.
//!
//! Results come from providers, see [`providers`]: the apps, a calculator
//! (`=`, or plain math like `2+2`), commands to run (`>`, Shift+Enter runs
//! them in a terminal), scripts from `config.toml`, and providers that
//! other modules and plugins offer. The apps and the built-ins answer at
//! once; the others a moment after typing stops, as they come.
//!
//! Apps and commands start through `uwsm app` in a uwsm session, otherwise
//! through `systemd-run --user --scope`, so they never belong to mochid.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.launcher]
//! # Runs terminal apps when uwsm doesn't. Defaults to xdg-terminal-exec when
//! # installed, otherwise $TERMINAL -e.
//! terminal = ["foot", "-e"]
//! max_results = 50
//!
//! [module.launcher.providers.web]
//! prefix = "!w"
//! command = ["web-search"]
//! ```

mod calc;
mod entries;
mod history;
mod launch;
mod providers;
mod script;
mod search;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, Contribution,
    Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::entries::{App, Locale};
use crate::history::History;
use crate::launch::Method;
use crate::providers::{Item, Kind, Provider, ProviderSettings, Verb};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long typing must pause before scripts and other modules are asked.
const DEBOUNCE: Duration = Duration::from_millis(120);
/// Commands the user ran, in the launch history.
const RAN: &str = "run:";

#[derive(Debug, Default)]
pub struct Launcher;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    terminal: Vec<String>,
    max_results: usize,
    providers: BTreeMap<String, ProviderSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            terminal: Vec::new(),
            max_results: 50,
            providers: BTreeMap::new(),
        }
    }
}

impl Module for Launcher {
    fn id(&self) -> &'static str {
        "launcher"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("toggle", "Open the launcher, or close it when open"),
            ActionSpec::new("open", "Open the launcher"),
            ActionSpec::new("close", "Close the launcher"),
            ActionSpec::new("search", "Search; the launcher sends this as you type").arg(
                ArgSpec::string("query", "What to look for")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new("launch", "Start an app by desktop id, like firefox.desktop")
                .arg(ArgSpec::string("id", "Desktop id, or id:action")),
            ActionSpec::new(
                "activate",
                "Pick a result; the launcher sends this on Enter or a click",
            )
            .arg(ArgSpec::string("key", "The result's key"))
            .arg(ArgSpec::bool("terminal", "Run a command in a terminal").optional()),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let method = Method::detect().await;
            tracing::info!(?method, "starting apps");
            let (sender, mut answers) = mpsc::unbounded_channel();
            let mut state = State {
                terminal: if settings.terminal.is_empty() {
                    default_terminal()
                } else {
                    settings.terminal.clone()
                },
                max_results: settings.max_results,
                method,
                history: History::default_path()
                    .map(History::load)
                    .unwrap_or_default(),
                providers: providers::providers(&settings.providers, &[]),
                settings: settings.providers,
                apps: Vec::new(),
                query: String::new(),
                shown: None,
                generation: 0,
                results: BTreeMap::new(),
                waiting: Vec::new(),
                due: None,
                asking: BTreeSet::new(),
                tasks: Vec::new(),
                listed: HashMap::new(),
                sender,
            };

            loop {
                let due = state.due;
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, .. }) if state.shown == Some(activity) => {
                            state.shown = None;
                            state.forget();
                        }
                        Some(ModuleEvent::Offers(offers)) => state.offered(&ctx, &offers),
                        Some(_) => {}
                    },
                    Some(answer) = answers.recv() => state.answered(&ctx, answer),
                    () = wait(due) => state.ask(&ctx),
                }
            }
        })
    }
}

async fn wait(due: Option<Instant>) {
    match due {
        Some(due) => tokio::time::sleep_until(due).await,
        None => std::future::pending().await,
    }
}

/// A script's or a module's answer to one query.
#[derive(Debug)]
struct Answer {
    generation: u64,
    provider: String,
    result: Result<String, String>,
}

#[derive(Debug)]
struct State {
    terminal: Vec<String>,
    max_results: usize,
    method: Method,
    history: History,
    settings: BTreeMap<String, ProviderSettings>,
    providers: Vec<Provider>,
    /// Read again every time the launcher opens, so new installs show up.
    apps: Vec<App>,
    query: String,
    shown: Option<ActivityId>,
    /// Counts queries, so answers to older ones are dropped.
    generation: u64,
    /// The results for the query, by provider.
    results: BTreeMap<String, Vec<Item>>,
    /// The providers to ask once typing pauses, with their part of the
    /// query.
    waiting: Vec<(Provider, String)>,
    due: Option<Instant>,
    /// The providers asked and not answered yet.
    asking: BTreeSet<String>,
    tasks: Vec<JoinHandle<()>>,
    /// The results on screen, by the key the view sends back.
    listed: HashMap<String, (String, Item)>,
    sender: mpsc::UnboundedSender<Answer>,
}

impl State {
    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let result = match command.action.as_str() {
            "toggle" if self.shown.is_some() => {
                self.close(ctx);
                Ok(())
            }
            "toggle" | "open" => {
                self.open(ctx).await;
                Ok(())
            }
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "search" => {
                self.query = command.args.str("query").unwrap_or_default().to_owned();
                if self.shown.is_some() {
                    self.search();
                    self.refresh(ctx);
                }
                Ok(())
            }
            "launch" => {
                let id = command.args.str("id").unwrap_or_default().to_owned();
                self.launch(ctx, &id).await
            }
            "activate" => {
                let key = command.args.str("key").unwrap_or_default().to_owned();
                let terminal = command.args.bool("terminal").unwrap_or(false);
                self.activate(ctx, &key, terminal).await
            }
            other => Err(format!("launcher has no action {other}")),
        };
        command.reply(result);
    }

    async fn open(&mut self, ctx: &ModuleCtx) {
        // The other panels take the keyboard too; only one can be
        // open. Not awaited: they close the launcher the same way.
        for module in ["hub", "clipboard", "audio", "tray"] {
            let close = ctx.call(module, "close", &[]);
            tokio::spawn(async move {
                match close.await {
                    Ok(()) | Err(CallError::NotEnabled(_)) => {}
                    Err(error) => tracing::warn!(%error, module, "could not close it"),
                }
            });
        }

        self.apps = read_apps().await;
        self.query.clear();
        self.search();
        let spec = ActivitySpec::new("Launcher")
            .key("launcher")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(self.payload(ctx));
        self.shown = Some(ctx.present(spec));
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown.take() {
            ctx.withdraw(id);
        }
        self.forget();
    }

    /// Drops the query's results and stops asking for more.
    fn forget(&mut self) {
        self.generation += 1;
        for task in self.tasks.drain(..) {
            task.abort();
        }
        self.results.clear();
        self.waiting.clear();
        self.asking.clear();
        self.due = None;
        self.listed.clear();
    }

    fn offered(&mut self, ctx: &ModuleCtx, offers: &[Contribution]) {
        self.providers = providers::providers(&self.settings, offers);
        tracing::info!(
            providers = ?self.providers.iter().map(|provider| &provider.name).collect::<Vec<_>>(),
            "providers"
        );
        if self.shown.is_some() {
            self.search();
            self.refresh(ctx);
        }
    }

    /// Answers the query: the built-ins at once, the others once typing
    /// pauses.
    fn search(&mut self) {
        self.forget();
        let query = self.query.clone();
        for (provider, rest) in providers::route(&self.providers, &query) {
            let prefixed = provider
                .prefix
                .as_deref()
                .is_some_and(|prefix| query.starts_with(prefix));
            let items = match &provider.kind {
                Kind::Apps => self.apps(&rest),
                Kind::Calculator => calculator(&rest, prefixed),
                Kind::Commands => self.commands(&rest),
                // Without a prefix, they have nothing to say to nothing.
                Kind::Script { .. } | Kind::Module { .. } => {
                    if prefixed || !rest.trim().is_empty() {
                        self.waiting.push((provider.clone(), rest));
                    }
                    continue;
                }
            };
            self.results.insert(provider.name.clone(), items);
        }
        if !self.waiting.is_empty() {
            self.due = Some(Instant::now() + DEBOUNCE);
        }
    }

    /// Asks the scripts and modules waiting for the query.
    fn ask(&mut self, ctx: &ModuleCtx) {
        self.due = None;
        for (provider, rest) in std::mem::take(&mut self.waiting) {
            let sender = self.sender.clone();
            let generation = self.generation;
            let name = provider.name.clone();
            self.asking.insert(name.clone());
            let task = match provider.kind {
                Kind::Script {
                    command,
                    timeout_ms,
                    ..
                } => tokio::spawn(async move {
                    let timeout = Duration::from_millis(timeout_ms);
                    let result =
                        script::run(&command, &rest, timeout, ("MOCHI_QUERY", &rest)).await;
                    let _ = sender.send(Answer {
                        generation,
                        provider: name,
                        result,
                    });
                }),
                Kind::Module { module, search, .. } => {
                    let args: Vec<&str> = if rest.is_empty() {
                        Vec::new()
                    } else {
                        vec![&rest]
                    };
                    let answer = ctx.ask(&module, &search, &args);
                    tokio::spawn(async move {
                        let result = answer
                            .await
                            .map(Option::unwrap_or_default)
                            .map_err(|error| error.to_string());
                        let _ = sender.send(Answer {
                            generation,
                            provider: name,
                            result,
                        });
                    })
                }
                Kind::Apps | Kind::Calculator | Kind::Commands => continue,
            };
            self.tasks.push(task);
        }
        self.refresh(ctx);
    }

    fn answered(&mut self, ctx: &ModuleCtx, answer: Answer) {
        if answer.generation != self.generation {
            return;
        }
        self.asking.remove(&answer.provider);
        let items = match answer.result {
            Ok(output) => {
                let (items, error) = providers::parse(&output, self.max_results);
                if let Some(error) = error {
                    tracing::warn!(provider = %answer.provider, "left out results it sent: {error}");
                }
                items
            }
            Err(error) => {
                tracing::warn!(provider = %answer.provider, %error, "the provider failed");
                Vec::new()
            }
        };
        self.results.insert(answer.provider, items);
        self.refresh(ctx);
    }

    fn refresh(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.shown {
            let payload = self.payload(ctx);
            ctx.update(id, payload);
        }
    }

    fn apps(&self, query: &str) -> Vec<Item> {
        search::rank(&self.apps, &self.history, query, now(), self.max_results)
            .into_iter()
            .map(|hit| Item {
                title: hit.name,
                subtitle: hit.description,
                icon: hit.icon,
                verb: Some(Verb::Launch(hit.id)),
                small: hit.action,
                ..Item::default()
            })
            .collect()
    }

    /// The command as typed first, then the ones run before that contain it,
    /// most used first.
    fn commands(&self, query: &str) -> Vec<Item> {
        let item = |command: &str, subtitle: &str| Item {
            title: command.to_owned(),
            subtitle: Some(subtitle.to_owned()),
            icon: Some("utilities-terminal".into()),
            verb: Some(Verb::Run(command.to_owned())),
            ..Item::default()
        };
        let query = query.trim();
        let mut items = Vec::new();
        if !query.is_empty() {
            items.push(item(query, "Run · Shift+Enter in a terminal"));
        }
        items.extend(
            self.history
                .starting_with(RAN, now())
                .iter()
                .filter(|command| command.as_str() != query && command.contains(query))
                .take(self.max_results)
                .map(|command| item(command, "Ran before")),
        );
        items
    }

    fn payload(&mut self, ctx: &ModuleCtx) -> Value {
        self.listed.clear();
        let mut results = Vec::new();
        let mut sections = 0;
        for provider in &self.providers {
            let Some(items) = self
                .results
                .get(&provider.name)
                .filter(|items| !items.is_empty())
            else {
                continue;
            };
            sections += 1;
            for (index, item) in items.iter().take(self.max_results).enumerate() {
                let key = format!("{}/{}/{index}", self.generation, provider.name);
                results.push(json!({
                    "key": key,
                    "title": item.title,
                    "subtitle": item.subtitle,
                    "icon": item.icon,
                    "glyph": item.glyph,
                    "small": item.small,
                    "section": provider.title,
                }));
                self.listed
                    .insert(key, (provider.name.clone(), item.clone()));
            }
        }
        json!({
            // Only the island on this monitor takes the keyboard.
            "output": ctx.compositor().state().focused_output,
            "query": self.query,
            "results": results,
            // Headings only help when there is more than one kind.
            "sections": sections > 1,
            "searching": self.due.is_some() || !self.asking.is_empty(),
        })
    }

    async fn launch(&mut self, ctx: &ModuleCtx, id: &str) -> Result<(), String> {
        if self.apps.is_empty() {
            self.apps = read_apps().await;
        }
        let (app_id, action_id) = match id.split_once(':') {
            Some((app, action)) => (app, Some(action)),
            None => (id, None),
        };
        let app = self
            .apps
            .iter()
            .find(|app| app.id == app_id)
            .ok_or_else(|| format!("no app {app_id}"))?;
        let action = match action_id {
            Some(action_id) => Some(
                app.actions
                    .iter()
                    .find(|action| action.id == action_id)
                    .ok_or_else(|| format!("{} has no action {action_id}", app.name))?,
            ),
            None => None,
        };
        launch::launch(self.method, app, action, &self.terminal)?;
        tracing::info!(id, "launched");
        self.history.record(app_id, now());
        self.close(ctx);
        Ok(())
    }

    /// Does what a result says, then tells its provider, when it asks to
    /// know.
    async fn activate(&mut self, ctx: &ModuleCtx, key: &str, terminal: bool) -> Result<(), String> {
        let (name, item) = self
            .listed
            .get(key)
            .cloned()
            .ok_or("that result is gone; search again")?;
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.name == name)
            .cloned();
        match item.verb.clone() {
            Some(Verb::Launch(id)) => return self.launch(ctx, &id).await,
            Some(Verb::Copy(text)) => {
                self.close(ctx);
                clipboard(ctx, "copy-text", text);
            }
            Some(Verb::Type(text)) => {
                // Closing gives the keyboard back to the window to type in.
                self.close(ctx);
                clipboard(ctx, "paste-text", text);
            }
            Some(Verb::Open(target)) => {
                launch::spawn(self.method, &["xdg-open".into(), target], None)?;
                self.close(ctx);
            }
            Some(Verb::Run(command)) => {
                let argv = ["sh".into(), "-c".into(), command.clone()];
                launch::spawn(self.method, &argv, terminal.then_some(&self.terminal[..]))?;
                tracing::info!(%command, terminal, "ran");
                self.history.record(&format!("{RAN}{command}"), now());
                self.close(ctx);
            }
            None => self.close(ctx),
        }
        if let (Some(provider), Some(id)) = (provider, item.id) {
            picked(ctx, &provider, id);
        }
        Ok(())
    }
}

/// The calculator's answer. With its prefix it says what's wrong; without,
/// it only answers what is plainly math.
fn calculator(query: &str, prefixed: bool) -> Vec<Item> {
    let query = query.trim();
    if query.is_empty() || (!prefixed && !calc::looks_like_math(query)) {
        return Vec::new();
    }
    let item = match calc::evaluate(query) {
        Ok(value) => {
            let value = calc::format(value);
            Item {
                title: value.clone(),
                subtitle: Some(format!("{query} · Enter copies it")),
                glyph: Some("=".into()),
                verb: Some(Verb::Copy(value)),
                ..Item::default()
            }
        }
        Err(error) => Item {
            title: query.to_owned(),
            subtitle: Some(error),
            glyph: Some("=".into()),
            ..Item::default()
        },
    };
    vec![item]
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

/// Tells a provider which of its results the user picked.
fn picked(ctx: &ModuleCtx, provider: &Provider, id: String) {
    match &provider.kind {
        Kind::Module {
            module,
            pick: Some(pick),
            ..
        } => {
            let call = ctx.call(module, pick, &[&id]);
            let module = module.clone();
            tokio::spawn(async move {
                if let Err(error) = call.await {
                    tracing::warn!(%error, %module, "could not tell it what was picked");
                }
            });
        }
        Kind::Script {
            pick, timeout_ms, ..
        } if !pick.is_empty() => {
            let pick = pick.clone();
            let timeout = Duration::from_millis(*timeout_ms);
            tokio::spawn(async move {
                if let Err(error) = script::run(&pick, &id, timeout, ("MOCHI_PICK", &id)).await {
                    tracing::warn!(%error, "the provider's pick command failed");
                }
            });
        }
        _ => {}
    }
}

async fn read_apps() -> Vec<App> {
    let desktops: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|desktop| !desktop.is_empty())
        .map(str::to_owned)
        .collect();
    let directories = entries::directories();
    tokio::task::spawn_blocking(move || entries::scan(&directories, &Locale::from_env(), &desktops))
        .await
        .unwrap_or_default()
}

/// `xdg-terminal-exec` when installed, otherwise `$TERMINAL -e`.
fn default_terminal() -> Vec<String> {
    if entries::installed("xdg-terminal-exec") {
        return vec!["xdg-terminal-exec".into()];
    }
    match std::env::var("TERMINAL") {
        Ok(terminal) if !terminal.is_empty() => vec![terminal, "-e".into()],
        _ => Vec::new(),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calculator_answers_math_and_explains_errors_after_its_prefix() {
        let answer = calculator("2+2", false);
        assert_eq!(answer[0].title, "4");
        assert_eq!(answer[0].verb, Some(Verb::Copy("4".into())));
        assert!(calculator("firefox", false).is_empty());
        assert!(calculator("2+", false).is_empty());
        let error = calculator("2+", true);
        assert_eq!(error[0].verb, None);
        assert_eq!(error[0].subtitle.as_deref(), Some("trailing operator"));
        assert!(calculator("", true).is_empty());
    }
}

/// The example scripts in `examples/launcher`, run the way the launcher
/// runs them.
#[cfg(test)]
mod example_scripts {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use crate::providers::{self, Verb};
    use crate::script;

    fn example(name: &str) -> Vec<String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/launcher")
            .join(name);
        vec!["sh".into(), path.display().to_string()]
    }

    #[tokio::test]
    async fn web_search_opens_a_search() {
        let output = script::run(
            &example("web-search.sh"),
            "rust \"lang\" & café",
            Duration::from_secs(5),
            ("MOCHI_QUERY", ""),
        )
        .await
        .unwrap();
        let (items, error) = providers::parse(&output, 10);
        assert_eq!(error, None);
        assert_eq!(items[0].title, "Search the web for rust \"lang\" & café");
        assert_eq!(
            items[0].verb,
            Some(Verb::Open(
                "https://duckduckgo.com/?q=rust%20%22lang%22%20%26%20caf%C3%A9".into()
            ))
        );
    }

    #[tokio::test]
    async fn files_finds_by_name_and_skips_hidden_ones() {
        let root: PathBuf =
            std::env::temp_dir().join(format!("mochi-files-{}", std::process::id()));
        std::fs::create_dir_all(root.join("notes/.hidden")).unwrap();
        for file in [
            "notes/Report \"q\".txt",
            "notes/.hidden/report.txt",
            "other.md",
        ] {
            std::fs::write(root.join(file), "").unwrap();
        }
        // The script reads FILES_ROOT from its environment.
        let command = vec![
            "env".into(),
            format!("FILES_ROOT={}", root.display()),
            "sh".into(),
            example("files.sh")[1].clone(),
        ];
        let output = script::run(
            &command,
            "report",
            Duration::from_secs(5),
            ("MOCHI_QUERY", ""),
        )
        .await
        .unwrap();
        let (items, error) = providers::parse(&output, 10);
        assert_eq!(error, None);
        assert_eq!(items.len(), 1, "{output}");
        assert_eq!(items[0].title, "Report \"q\".txt");
        let expected = root.join("notes/Report \"q\".txt").display().to_string();
        assert_eq!(items[0].verb, Some(Verb::Open(expected)));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "launcher",
            include_str!("../settings.toml"),
        );
    }
}

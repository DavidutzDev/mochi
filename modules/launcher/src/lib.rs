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
mod files;
mod history;
mod launch;
mod providers;
mod script;
mod search;
mod tour;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, CallError, Contribution,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::files::FilesSettings;
use crate::history::History;
use crate::launch::Method;
use crate::providers::{Engine, Item, Kind, Provider, ProviderSettings, Verb};
use mochi_core::desktop::{self, App, Locale};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long typing must pause before scripts are asked. Modules are asked
/// at once: answering is cheap for them.
const DEBOUNCE: Duration = Duration::from_millis(120);
/// Commands the user ran, in the launch history.
const RAN: &str = "run:";

#[derive(Debug, Default)]
pub struct Launcher;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    #[schemars(extend("x-suggest" = [["kitty"], ["foot"], ["alacritty"], ["ghostty"], ["wezterm", "start"], ["konsole"], ["gnome-terminal", "--"], ["xterm"]]))]
    terminal: Vec<String>,
    max_results: usize,
    layout: Layout,
    providers: BTreeMap<String, ProviderSettings>,
    engines: BTreeMap<String, Engine>,
    files: FilesSettings,
}

/// How the results are listed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Layout {
    /// Two lines per result: the name, and its description under it.
    #[default]
    Comfortable,
    /// One line per result, the icon and the name; the selected one shows
    /// its description at the end of its line.
    Compact,
}

impl Layout {
    fn as_str(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            terminal: Vec::new(),
            max_results: 50,
            layout: Layout::Comfortable,
            providers: BTreeMap::new(),
            engines: BTreeMap::new(),
            files: FilesSettings::default(),
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

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = tour::steps();
        offers.push(ContributionSpec::preload("Launcher"));
        offers
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn needs(&self, table: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        let settings: Settings = mochi_core::settings(table).unwrap_or_default();
        let mut needs = vec![
            mochi_core::Need::new("xdg-open", "Opening files and web searches"),
            mochi_core::Need::new(
                "wl-copy",
                "Copying results, while the clipboard module is off",
            ),
        ];
        if let Some(terminal) = settings.terminal.first() {
            needs.push(mochi_core::Need::new(
                terminal,
                "Running commands in a terminal",
            ));
        }
        needs
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("toggle", "Open the launcher, or close it when open"),
            ActionSpec::new("open", "Open the launcher, with something typed already").arg(
                ArgSpec::string("query", "What's typed in it, like : for emoji")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new("close", "Close the launcher"),
            ActionSpec::new("search", "Search; the launcher sends this as you type").arg(
                ArgSpec::string("query", "What to look for")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new("launch", "Start an app by desktop id, like firefox.desktop")
                .arg(ArgSpec::string("id", "Desktop id, or id:action").source("desktop-id")),
            ActionSpec::new(
                "activate",
                "Pick a result; the launcher sends this on Enter or a click",
            )
            .arg(ArgSpec::string("key", "The result's key"))
            .arg(
                ArgSpec::bool(
                    "alternate",
                    "Do what Shift+Enter does: run a command in a terminal, copy an emoji, show a file in its folder",
                )
                .optional(),
            ),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let method = Method::detect().await;
            tracing::info!(?method, "starting apps");
            let (sender, mut answers) = mpsc::unbounded_channel();
            let (indexed, mut indexes) = mpsc::unbounded_channel();
            let engines = providers::engines(&settings.engines);
            let mut state = State {
                terminal: if settings.terminal.is_empty() {
                    default_terminal()
                } else {
                    settings.terminal.clone()
                },
                max_results: settings.max_results,
                layout: settings.layout,
                method,
                history: History::default_path()
                    .map(History::load)
                    .unwrap_or_default(),
                providers: providers::providers(&settings.providers, &engines, &[]),
                settings: settings.providers,
                engines,
                files: settings.files,
                index: files::Index::default(),
                indexing: false,
                files_asked: false,
                watching: true,
                indexed,
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
            state.reindex();

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
                    Some(update) = indexes.recv() => state.indexed(&ctx, update),
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
    layout: Layout,
    method: Method,
    history: History,
    settings: BTreeMap<String, ProviderSettings>,
    engines: BTreeMap<String, Engine>,
    files: FilesSettings,
    index: files::Index,
    indexing: bool,
    /// The query asks the files provider, which may still be indexing.
    files_asked: bool,
    /// Whether to follow changes with inotify. Turned off for good when
    /// the system runs out of watches.
    watching: bool,
    indexed: mpsc::UnboundedSender<files::Update>,
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
                let query = command.args.str("query").unwrap_or_default().to_owned();
                self.open(ctx, query).await;
                Ok(())
            }
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "search" => {
                self.query = command.args.str("query").unwrap_or_default().to_owned();
                if self.shown.is_some() {
                    self.search(ctx);
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
                let alternate = command.args.bool("alternate").unwrap_or(false);
                self.activate(ctx, &key, alternate).await
            }
            other => Err(format!("launcher has no action {other}")),
        };
        command.reply(result);
    }

    async fn open(&mut self, ctx: &ModuleCtx, query: String) {
        ctx.close_other_panels();

        self.apps = read_apps().await;
        if self.index.stale() {
            self.reindex();
        }
        self.query = query;
        self.search(ctx);
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
        self.files_asked = false;
        self.listed.clear();
    }

    /// Builds the file index again, in the background. While its watches
    /// follow changes, only the first time.
    fn reindex(&mut self) {
        if self.indexing
            || !self
                .providers
                .iter()
                .any(|provider| provider.kind == Kind::Files)
        {
            return;
        }
        self.indexing = true;
        files::spawn(self.files.clone(), self.watching, self.indexed.clone());
    }

    fn indexed(&mut self, ctx: &ModuleCtx, update: files::Update) {
        match update {
            files::Update::Built { entries, live } => {
                tracing::info!(entries = entries.len(), live, "indexed files");
                self.index = files::Index::new(entries, live, self.files.max);
                self.watching = live;
                self.indexing = false;
            }
            files::Update::Changed(changes) => {
                tracing::debug!(changes = changes.len(), "files changed");
                self.index.apply(changes);
            }
            files::Update::Lost => {
                self.index.lost();
                self.watching = false;
            }
        }
        if self.shown.is_some() && self.query.starts_with('/') {
            self.search(ctx);
            self.refresh(ctx);
        }
    }

    fn offered(&mut self, ctx: &ModuleCtx, offers: &[Contribution]) {
        self.providers = providers::providers(&self.settings, &self.engines, offers);
        tracing::info!(
            providers = ?self.providers.iter().map(|provider| &provider.name).collect::<Vec<_>>(),
            "providers"
        );
        if self.shown.is_some() {
            self.search(ctx);
            self.refresh(ctx);
        }
    }

    /// Answers the query: the built-ins and modules at once, scripts once
    /// typing pauses.
    fn search(&mut self, ctx: &ModuleCtx) {
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
                Kind::Files => {
                    self.files_asked = true;
                    self.files(&rest)
                }
                Kind::Windows => windows(&ctx.compositor().toplevels(), &rest, self.max_results),
                Kind::Web { url } => web(&provider.title, url, &rest),
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
        // Modules now, scripts after the pause.
        let (modules, scripts): (Vec<_>, Vec<_>) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|(provider, _)| matches!(provider.kind, Kind::Module { .. }));
        self.waiting = modules;
        if !self.waiting.is_empty() {
            self.ask(ctx);
        }
        self.waiting = scripts;
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
                Kind::Apps
                | Kind::Calculator
                | Kind::Commands
                | Kind::Files
                | Kind::Windows
                | Kind::Web { .. } => {
                    continue;
                }
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
            alt: Some(Verb::RunInTerminal(command.to_owned())),
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

    /// Files and folders by name. Enter opens one, Shift+Enter shows it
    /// in its folder, and it drags out onto other apps.
    fn files(&self, query: &str) -> Vec<Item> {
        self.index
            .search(query, self.max_results)
            .into_iter()
            .map(|entry| {
                let path = entry.path.display().to_string();
                Item {
                    title: entry
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    subtitle: entry.path.parent().map(files::short),
                    icon: Some(
                        if entry.folder {
                            "folder"
                        } else {
                            "description"
                        }
                        .into(),
                    ),
                    verb: Some(Verb::Open(path.clone())),
                    alt: Some(Verb::Show(path.clone())),
                    file: Some(path),
                    ..Item::default()
                }
            })
            .collect()
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
                    "color": item.color.as_deref().map(qml_color),
                    "small": item.small,
                    "file": item.file,
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
            "layout": self.layout.as_str(),
            "results": results,
            // Headings only help when there is more than one kind.
            "sections": sections > 1,
            // Waiting on scripts, modules, or the file index being built.
            "searching": self.due.is_some() || !self.asking.is_empty() || (self.files_asked && self.indexing),
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
    async fn activate(
        &mut self,
        ctx: &ModuleCtx,
        key: &str,
        alternate: bool,
    ) -> Result<(), String> {
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
        // Shift+Enter does the other thing, when the result has one.
        let verb = match (alternate, &item.alt) {
            (true, Some(alt)) => Some(alt.clone()),
            _ => item.verb.clone(),
        };
        match verb {
            Some(Verb::Launch(id)) => return self.launch(ctx, &id).await,
            Some(Verb::Focus(id)) => {
                self.close(ctx);
                // Once the island has let go of the keyboard: the
                // compositor gives it back to the window focused before,
                // which would undo an activation sent sooner.
                let compositor = ctx.compositor().clone();
                tokio::spawn(async move {
                    tokio::time::sleep(FOCUS_DELAY).await;
                    if let Err(error) = compositor.activate_toplevel(id) {
                        tracing::warn!(%error, "can't focus the window");
                    }
                });
            }
            Some(Verb::Copy(text)) => {
                self.close(ctx);
                clipboard(ctx, "copy-text", text);
            }
            Some(Verb::Type(text)) => {
                // Closing gives the keyboard back to the window to type in.
                self.close(ctx);
                clipboard(ctx, "paste-text", text);
            }
            Some(Verb::Show(path)) => {
                self.close(ctx);
                let paths = [PathBuf::from(path)];
                tokio::spawn(async move {
                    if let Err(error) = mochi_core::process::show_in_folder(&paths).await {
                        tracing::warn!(%error, "can't show the file in its folder");
                    }
                });
            }
            Some(Verb::Open(target)) => {
                launch::spawn(self.method, &["xdg-open".into(), target], None)?;
                self.close(ctx);
            }
            Some(verb @ (Verb::Run(_) | Verb::RunInTerminal(_))) => {
                let terminal = matches!(verb, Verb::RunInTerminal(_));
                let (Verb::Run(command) | Verb::RunInTerminal(command)) = verb else {
                    unreachable!("matched above");
                };
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

/// How long after closing the launcher a picked window gets focus.
const FOCUS_DELAY: Duration = Duration::from_millis(150);

/// A CSS color as QML reads it. CSS puts a hex color's alpha last
/// (`#rrggbbaa`, `#rgba`), QML first (`#aarrggbb`); the rest is the same.
fn qml_color(css: &str) -> String {
    let Some(hex) = css.strip_prefix('#') else {
        return css.to_owned();
    };
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return css.to_owned();
    }
    match hex.len() {
        8 => format!("#{}{}", &hex[6..], &hex[..6]),
        4 => {
            let doubled: String = hex.chars().flat_map(|c| [c, c]).collect();
            format!("#{}{}", &doubled[6..], &doubled[..6])
        }
        _ => css.to_owned(),
    }
}

/// The open windows whose title or app has every word of `query`, the one
/// focused most recently first. Nothing for an empty query: apps come
/// first then.
fn windows(open: &[mochi_core::compositor::Toplevel], query: &str, max: usize) -> Vec<Item> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Vec::new();
    }
    open.iter()
        .filter(|window| {
            let text = format!("{} {}", window.title, window.app_id).to_lowercase();
            words.iter().all(|word| text.contains(word))
        })
        .take(max)
        .map(|window| Item {
            title: if window.title.is_empty() {
                window.app_id.clone()
            } else {
                window.title.clone()
            },
            subtitle: Some(if window.focused {
                format!("{} · focused", window.app_id)
            } else {
                window.app_id.clone()
            }),
            icon: Some(window.app_id.to_lowercase()).filter(|icon| !icon.is_empty()),
            verb: Some(Verb::Focus(window.id)),
            ..Item::default()
        })
        .collect()
}

/// A web search: the query as one result that opens the engine's page.
fn web(title: &str, url: &str, query: &str) -> Vec<Item> {
    let query = query.trim();
    if query.is_empty() {
        return vec![Item {
            title: format!("Type to search {title}"),
            icon: Some("web-browser".into()),
            ..Item::default()
        }];
    }
    let page = providers::web_url(url, query);
    vec![Item {
        title: query.to_owned(),
        subtitle: Some(format!("Search {title}")),
        icon: Some("web-browser".into()),
        verb: Some(Verb::Open(page)),
        ..Item::default()
    }]
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
    let desktops = desktop::current_desktops();
    let directories = desktop::directories();
    tokio::task::spawn_blocking(move || desktop::scan(&directories, &Locale::from_env(), &desktops))
        .await
        .unwrap_or_default()
}

/// `xdg-terminal-exec` when installed, otherwise `$TERMINAL -e`.
fn default_terminal() -> Vec<String> {
    if desktop::installed("xdg-terminal-exec") {
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
    fn hex_alpha_moves_to_the_front_for_qml() {
        assert_eq!(qml_color("#f38ba880"), "#80f38ba8");
        assert_eq!(qml_color("#abc8"), "#88aabbcc");
        assert_eq!(qml_color("#1e1e2e"), "#1e1e2e");
        assert_eq!(qml_color("rebeccapurple"), "rebeccapurple");
        assert_eq!(qml_color("#zzzzzzzz"), "#zzzzzzzz");
    }

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

#[cfg(test)]
mod windows_tests {
    use mochi_core::compositor::Toplevel;

    use super::*;

    #[test]
    fn windows_match_their_title_and_app() {
        let window = |id, title: &str, app_id: &str, focused| Toplevel {
            id,
            title: title.into(),
            app_id: app_id.into(),
            focused,
        };
        let open = [
            window(3, "Docs - Mozilla Firefox", "firefox", true),
            window(7, "~/mochi", "kitty", false),
            window(9, "", "Spotify", false),
        ];
        assert!(windows(&open, "", 10).is_empty());
        let found = windows(&open, "fire docs", 10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].verb, Some(Verb::Focus(3)));
        assert_eq!(found[0].subtitle.as_deref(), Some("firefox · focused"));
        let untitled = windows(&open, "spot", 10);
        assert_eq!(untitled[0].title, "Spotify");
        assert_eq!(untitled[0].icon.as_deref(), Some("spotify"));
    }
}

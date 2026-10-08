//! The daemon loop. It owns the arbiter, the bubbles, the module slots and
//! the connections, and is the only place any of them change.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mochi_core::actions;
use mochi_core::compositor::Compositor;
use mochi_core::supervisor::{Supervisor, UiEvent};
use mochi_core::{
    ActivitySpec, Arbiter, Assets, Bubbles, CallError, ClickOutside, Config, Effect, Module,
    ModuleCommand, ModuleError, ModuleEvent, ModuleRequest, Notices, Panels, Priority, Reply,
    Request, SettingsOp,
};
use mochi_plugins::manifest::CORE_ID;
use mochi_protocol::{
    API, ActionSpec, ActivityId, Area, ClientMessage, CompositorStatus, Contribution,
    DaemonMessage, ErrorCode, EventKind, ModuleActions, PluginState, PluginStatus, Role, Status,
    Theme,
};
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task::JoinError;

use crate::ipc::{ConnectionEvent, ConnectionId};
use crate::modules::{self, Runner};
use crate::settings::{self, Store};

/// The module that shows the settings panel, which hears about changes.
const SETTINGS: &str = "settings";

/// How long a new Quickshell process gets to say hello.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long modules get to finish when the daemon stops.
const SHUTDOWN: Duration = Duration::from_secs(2);

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A running module, as the loop sees it.
#[derive(Debug)]
pub struct ModuleSlot {
    pub assets: Assets,
    pub actions: Vec<ActionSpec>,
    pub contributions: Vec<Contribution>,
    /// Which start of the module this is, to tell its exit from an earlier
    /// run's after a reload.
    pub generation: u64,
    /// `None` once the module's task has ended.
    pub events: Option<UnboundedSender<ModuleEvent>>,
}

/// Why a command never reached its module.
#[derive(Debug)]
enum Undelivered {
    UnknownModule,
    UnknownAction,
    InvalidArgs(String),
    NotRunning,
}

/// A module's task ended: its id, its generation, and how it ended.
pub type ModuleExit = (
    &'static str,
    u64,
    Result<Result<(), ModuleError>, JoinError>,
);

/// Everything the loop listens to.
#[derive(Debug)]
pub struct Inputs {
    pub connections: UnboundedReceiver<ConnectionEvent>,
    pub requests: UnboundedReceiver<ModuleRequest>,
    pub exits: UnboundedReceiver<ModuleExit>,
    pub ui: UnboundedReceiver<UiEvent>,
}

#[derive(Debug)]
struct Client {
    sender: UnboundedSender<DaemonMessage>,
    role: Option<Role>,
}

#[derive(Debug)]
pub struct Daemon {
    /// Enabled module ids, in config order.
    order: Vec<&'static str>,
    modules: BTreeMap<&'static str, ModuleSlot>,
    /// The settings each running module started with, to tell on reload
    /// which ones changed.
    settings: BTreeMap<&'static str, mochi_core::toml::Table>,
    states: BTreeMap<String, Value>,
    arbiter: Arbiter,
    bubbles: Bubbles,
    clients: HashMap<ConnectionId, Client>,
    theme: Theme,
    /// The config the modules run with.
    config: Config,
    files: Files,
    /// The files as read and the settings panel's changes over them.
    store: Store,
    runner: Runner,
    /// Attached once the shell is written; `None` only during startup.
    supervisor: Option<Supervisor>,
    /// The modules whose views the running Quickshell has.
    shell: Vec<&'static str>,
    /// The builtin views plugins replace, by module: file name and the
    /// plugin's file.
    overrides: BTreeMap<&'static str, Vec<(String, PathBuf)>>,
    /// Each running plugin's revision, to restart it when its files change.
    revisions: BTreeMap<&'static str, String>,
    /// The plugins plugins.toml lists, for `mochi status`.
    listed: Vec<modules::Listed>,
    /// Plugins whose backend gave up, and why.
    failed: BTreeMap<&'static str, String>,
    /// Who watches whose state: watched module to watchers.
    watchers: BTreeMap<String, BTreeSet<&'static str>>,
    /// What each module was last told is offered to it.
    offered: BTreeMap<&'static str, Vec<Contribution>>,
    compositor: Compositor,
    /// `[island] panels`: which monitor panels open on.
    panels: Panels,
    /// `[island] notices`: which monitor everything else shows on.
    notices: Notices,
    handshake_deadline: Option<Instant>,
    /// The list of an area's hidden bubbles, while the island has it.
    hidden_list: Option<(ActivityId, Area)>,
}

/// Where the daemon reads its configuration from.
#[derive(Debug)]
pub struct Files {
    pub config: PathBuf,
}

impl Daemon {
    pub fn new(runner: Runner, files: Files, store: Store, loaded: settings::Loaded) -> Self {
        let compositor = runner.compositor.clone();
        Self {
            order: Vec::new(),
            modules: BTreeMap::new(),
            settings: BTreeMap::new(),
            states: BTreeMap::new(),
            arbiter: Arbiter::new(),
            bubbles: Bubbles::default(),
            clients: HashMap::new(),
            theme: loaded.theme,
            config: loaded.config,
            files,
            store,
            runner,
            supervisor: None,
            shell: Vec::new(),
            overrides: BTreeMap::new(),
            revisions: BTreeMap::new(),
            listed: Vec::new(),
            failed: BTreeMap::new(),
            watchers: BTreeMap::new(),
            offered: BTreeMap::new(),
            compositor,
            panels: Panels::default(),
            notices: Notices::default(),
            handshake_deadline: None,
            hidden_list: None,
        }
    }

    pub fn attach(&mut self, supervisor: Supervisor) {
        self.supervisor = Some(supervisor);
    }

    /// Makes the running modules match `config`: starts new ones, stops
    /// removed ones, restarts the ones whose settings or plugin files changed
    /// and the ones that ended, and rewrites the shell's views. Modules that
    /// didn't change keep running.
    pub fn apply(&mut self, config: &Config) -> anyhow::Result<()> {
        let catalog = modules::catalog(&self.files.config)?;
        let mut builtin = catalog.modules;
        let wanted: Vec<&'static str> = config
            .modules
            .iter()
            .filter_map(|id| {
                let found = builtin
                    .iter()
                    .find(|module| module.id() == id)
                    .map(|module| module.id());
                if found.is_none() {
                    tracing::warn!(module = %id, "enabled, but the plugin can't load; see `mochi status`");
                }
                found
            })
            .collect();
        let revisions: BTreeMap<&'static str, String> = catalog
            .plugins
            .iter()
            .map(|(id, plugin)| (*id, plugin.revision()))
            .collect();

        let mut updated = false;
        for id in self.order.clone() {
            let changed = self.settings.get(id) != Some(&config.settings(id));
            let replaced = self.revisions.get(id) != revisions.get(id);
            // One that stopped or failed gets another go.
            let ended = self
                .modules
                .get(id)
                .is_some_and(|slot| slot.events.is_none());
            if !wanted.contains(&id) || changed || replaced || ended {
                self.stop(id);
                updated |= replaced;
            }
        }

        let overrides = overrides(&catalog.plugins, &wanted, &builtin);
        let views: Vec<&dyn Module> = wanted
            .iter()
            .filter_map(|id| builtin.iter().find(|module| module.id() == *id))
            .map(|module| module.as_ref())
            .collect();
        self.runner.write_shell(&views, &overrides)?;
        // A module new to the shell needs a fresh Quickshell to find its
        // views' types, and so do new overrides and a plugin's new files.
        // At startup none runs yet.
        let added = wanted.iter().any(|id| !self.shell.contains(id));
        let reload = added || updated || overrides != self.overrides;
        self.shell = wanted.clone();
        self.overrides = overrides;
        if reload && let Some(supervisor) = &self.supervisor {
            supervisor.reload();
        }

        for id in &wanted {
            if self.modules.contains_key(id) {
                continue;
            }
            let index = builtin
                .iter()
                .position(|module| module.id() == *id)
                .expect("found above");
            let settings = config.settings(id);
            let slot = self
                .runner
                .start(builtin.swap_remove(index), settings.clone())?;
            self.modules.insert(id, slot);
            self.settings.insert(id, settings);
            self.failed.remove(id);
            if let Some(revision) = revisions.get(id) {
                self.revisions.insert(id, revision.clone());
            }
            tracing::info!(module = id, "started");
        }
        self.order = wanted;
        self.listed = catalog.listed;
        self.panels = config.island.panels;
        self.notices = config.island.notices;
        self.arbiter
            .set_outside_expanded_only(config.island.click_outside == ClickOutside::Expanded);
        self.bubbles.configure(
            config.bubbles.modules.clone(),
            Some(config.bubbles.max_per_area),
        );
        self.bubbles.set_stack(config.bubbles.stacking());
        self.tell_offers();
        Ok(())
    }

    /// Tells each module what the others offer it, when that changed or it
    /// just started.
    fn tell_offers(&mut self) {
        let contributions = self.contributions();
        for module in self.order.clone() {
            let offers: Vec<Contribution> = contributions
                .iter()
                .filter(|contribution| contribution.target == module)
                .cloned()
                .collect();
            if self.offered.get(module) != Some(&offers) {
                self.notify(module, ModuleEvent::Offers(offers.clone()));
                self.offered.insert(module, offers);
            }
        }
    }

    /// Stops a module: its events end, which ends its task, and everything
    /// it showed goes away now.
    fn stop(&mut self, module: &'static str) {
        if self.modules.remove(module).is_none() {
            return;
        }
        self.settings.remove(module);
        self.revisions.remove(module);
        self.offered.remove(module);
        self.order.retain(|id| *id != module);
        for watchers in self.watchers.values_mut() {
            watchers.remove(module);
        }
        let now = Instant::now();
        self.arbiter.withdraw_all(module, now);
        self.bubbles.hide_all(module);
        // A module that paused the island can't resume it any more.
        if self.arbiter.exclusive() == Some(module) {
            self.arbiter.set_exclusive(None, now);
            self.bubbles.set_only(None);
        }
        if self.states.remove(module).is_some() {
            self.broadcast(&DaemonMessage::State {
                module: module.to_owned(),
                state: Value::Null,
            });
            self.tell_watchers(module, &Value::Null);
        }
        tracing::info!(module, "stopped");
    }

    /// Runs until SIGINT or SIGTERM.
    pub async fn run(mut self, mut inputs: Inputs) -> anyhow::Result<()> {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

        loop {
            let deadline = [self.arbiter.next_deadline(), self.handshake_deadline]
                .into_iter()
                .flatten()
                .min();
            let sleep = async {
                match deadline {
                    Some(deadline) => {
                        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
                    }
                    None => std::future::pending().await,
                }
            };

            tokio::select! {
                Some(event) = inputs.connections.recv() => self.on_connection(event),
                Some(request) = inputs.requests.recv() => {
                    // Take everything modules sent together, like the several
                    // withdrawals of a `clear`, so the island goes straight to
                    // the end result instead of flashing what lies between.
                    self.on_request(request);
                    while let Ok(request) = inputs.requests.try_recv() {
                        self.on_request(request);
                    }
                }
                Some((module, generation, result)) = inputs.exits.recv() => {
                    self.on_exit(module, generation, result);
                }
                Some(event) = inputs.ui.recv() => self.on_ui_process(event),
                () = sleep => {}
                _ = tokio::signal::ctrl_c() => break,
                _ = terminate.recv() => break,
            }

            let now = Instant::now();
            self.arbiter.tick(now);
            if self
                .handshake_deadline
                .is_some_and(|deadline| deadline <= now)
            {
                tracing::warn!("quickshell never said hello, restarting it");
                self.handshake_deadline = None;
                if let Some(supervisor) = &self.supervisor {
                    supervisor.restart();
                }
            }
            self.apply_effects();
        }

        tracing::info!("shutting down");
        if let Some(supervisor) = &self.supervisor {
            supervisor.stop();
        }
        // Closing the event channels ends every module's `next_event`. They
        // get a moment to clean up, like the share module removing its
        // monitor, before the runtime stops their tasks.
        let running = self.modules.len();
        self.modules.clear();
        let finished = tokio::time::timeout(SHUTDOWN, async {
            for _ in 0..running {
                if inputs.exits.recv().await.is_none() {
                    return;
                }
            }
        })
        .await;
        if finished.is_err() {
            tracing::warn!("some modules didn't stop in time");
        }
        Ok(())
    }

    fn on_connection(&mut self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Opened { id, sender } => {
                self.clients.insert(id, Client { sender, role: None });
            }
            ConnectionEvent::Message { id, message } => self.on_message(id, message),
            ConnectionEvent::Malformed { id, error } => {
                tracing::warn!(connection = id, %error, "malformed message");
                self.reply_error(id, ErrorCode::BadMessage, error);
            }
            ConnectionEvent::Closed { id } => {
                if let Some(Client {
                    role: Some(Role::Ui),
                    ..
                }) = self.clients.remove(&id)
                {
                    tracing::info!(connection = id, "ui disconnected");
                }
            }
        }
    }

    fn on_message(&mut self, id: ConnectionId, message: ClientMessage) {
        let Some(client) = self.clients.get(&id) else {
            return;
        };

        match (client.role, message) {
            (None, ClientMessage::Hello { api, role }) => self.on_hello(id, api, role),
            (None, _) => {
                self.reply_error(id, ErrorCode::HelloFirst, "send hello first");
                self.clients.remove(&id);
            }
            (Some(_), ClientMessage::Hello { .. }) => {
                self.reply_error(id, ErrorCode::BadMessage, "hello was already sent");
            }
            (Some(Role::Ui), ClientMessage::Event { activity, kind }) => {
                let now = Instant::now();
                match kind {
                    EventKind::Click => self.arbiter.click(activity, now),
                    EventKind::HoverEnter => self.arbiter.hover(activity, true, now),
                    EventKind::HoverLeave => self.arbiter.hover(activity, false, now),
                    EventKind::Dismiss => self.arbiter.dismiss(activity, now),
                    EventKind::Outside => self.arbiter.outside(activity, now),
                }
            }
            (Some(Role::Ui), ClientMessage::BubbleClick { bubble }) => {
                // A click in the list of hidden bubbles closes it, so a
                // panel the bubble opens doesn't wait behind it.
                if let Some((list, _)) = self.hidden_list {
                    self.arbiter.dismiss(list, Instant::now());
                }
                // Clicks on a bubble that just went away are dropped.
                if let Some(module) = self.bubbles.owner(bubble) {
                    let module = module.to_owned();
                    self.notify(&module, ModuleEvent::BubbleClicked(bubble));
                }
            }
            (Some(Role::Ui), ClientMessage::OverflowClick { area }) => self.list_hidden(area),
            (
                Some(_),
                ClientMessage::Event { .. }
                | ClientMessage::BubbleClick { .. }
                | ClientMessage::OverflowClick { .. },
            ) => {
                self.reply_error(id, ErrorCode::NotAllowed, "only the ui sends events");
            }
            (
                Some(_),
                ClientMessage::Command {
                    module,
                    action,
                    args,
                },
            ) => {
                self.on_command(id, &module, &action, &args);
            }
            (Some(_), ClientMessage::Status) => {
                let status = Status {
                    version: VERSION.into(),
                    api: API,
                    ui_connected: self.ui_connected(),
                    modules: self.order.iter().map(|id| (*id).to_owned()).collect(),
                    compositor: self.compositor_status(),
                    plugins: self.plugin_status(),
                };
                self.reply(id, DaemonMessage::Status { status });
            }
            (Some(_), ClientMessage::ListActions { module }) => self.on_list_actions(id, module),
            (Some(_), ClientMessage::Reload) => self.on_reload(id),
            (Some(_), ClientMessage::Dismiss) => {
                if let Some(shown) = self.arbiter.shown() {
                    self.arbiter.dismiss(shown.id, Instant::now());
                }
                self.reply(id, DaemonMessage::Ok);
            }
        }
    }

    /// Opens the list of an area's hidden bubbles in the island, or closes
    /// it when it already lists them.
    fn list_hidden(&mut self, area: Area) {
        let now = Instant::now();
        if let Some((list, listed)) = self.hidden_list
            && listed == area
        {
            self.arbiter.dismiss(list, now);
            return;
        }
        let bubbles = self.bubbles.hidden(area);
        if bubbles.is_empty() {
            return;
        }
        let mut spec = ActivitySpec::new(HIDDEN_VIEW)
            .key("hidden-bubbles")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(hidden_payload(area, &bubbles));
        spec.output = self.panel_output();
        let id = self.runner.ids.next();
        self.arbiter.submit(id, CORE_ID, spec, now);
        self.hidden_list = Some((id, area));
    }

    /// Keeps the open list of hidden bubbles up to date, and closes it when
    /// its area has none left.
    fn update_hidden_list(&mut self) {
        let Some((list, area)) = self.hidden_list else {
            return;
        };
        let bubbles = self.bubbles.hidden(area);
        let result = if bubbles.is_empty() {
            self.hidden_list = None;
            self.arbiter.withdraw(CORE_ID, list, Instant::now())
        } else {
            self.arbiter
                .update(CORE_ID, list, hidden_payload(area, &bubbles))
        };
        if let Err(error) = result {
            tracing::debug!(%error, "the list of hidden bubbles is gone");
            self.hidden_list = None;
        }
    }

    fn on_hello(&mut self, id: ConnectionId, api: u32, role: Role) {
        if api != API {
            let message = format!("this daemon speaks api {API}, the client speaks {api}");
            self.reply_error(id, ErrorCode::UnsupportedApi, message);
            self.clients.remove(&id);
            return;
        }
        if let Some(client) = self.clients.get_mut(&id) {
            client.role = Some(role);
        }
        self.reply(
            id,
            DaemonMessage::Hello {
                api: API,
                version: VERSION.into(),
            },
        );

        if role == Role::Ui {
            tracing::info!(connection = id, "ui connected");
            self.handshake_deadline = None;
            let modules = self.order.iter().map(|id| (*id).to_owned()).collect();
            self.reply(id, DaemonMessage::Modules { modules });
            let contributions = self.contributions();
            self.reply(id, DaemonMessage::Contributions { contributions });
            let theme = self.theme.clone();
            self.reply(
                id,
                DaemonMessage::Theme {
                    theme: Box::new(theme),
                },
            );
            for (module, state) in self.states.clone() {
                self.reply(id, DaemonMessage::State { module, state });
            }
            let activity = self.arbiter.shown();
            let resting = self.arbiter.resting();
            self.reply(id, DaemonMessage::Present { activity, resting });
            self.reply(id, self.bubbles_message());
        }
    }

    fn on_command(&mut self, id: ConnectionId, module: &str, action: &str, args: &[String]) {
        let reply = match self.deliver(module, action, args) {
            Ok(reply) => reply,
            Err(undelivered) => {
                let (code, message) = match undelivered {
                    Undelivered::UnknownModule => {
                        let enabled = self.order.join(", ");
                        let message =
                            format!("no module {module:?} is enabled (enabled: {enabled})");
                        (ErrorCode::UnknownModule, message)
                    }
                    Undelivered::UnknownAction => {
                        let message =
                            format!("{module} has no action {action:?}, see `mochi ipc {module}`");
                        (ErrorCode::UnknownAction, message)
                    }
                    Undelivered::InvalidArgs(message) => (ErrorCode::InvalidArgs, message),
                    Undelivered::NotRunning => {
                        (ErrorCode::ModuleFailed, format!("{module} is not running"))
                    }
                };
                self.reply_error(id, code, message);
                return;
            }
        };

        // Answer when the module does, without holding up the loop.
        let Some(sender) = self.clients.get(&id).map(|client| client.sender.clone()) else {
            return;
        };
        tokio::spawn(async move {
            let answer = match reply.await {
                Ok(Ok(None)) => DaemonMessage::Ok,
                Ok(Ok(Some(output))) => DaemonMessage::Output { output },
                Ok(Err(message)) => DaemonMessage::Error {
                    code: ErrorCode::ModuleFailed,
                    message,
                },
                Err(_) => DaemonMessage::Error {
                    code: ErrorCode::ModuleFailed,
                    message: "the module dropped the command without answering".into(),
                },
            };
            let _ = sender.send(answer);
        });
    }

    /// A module running another module's action.
    fn on_call(
        &self,
        caller: &str,
        module: &str,
        action: &str,
        args: &[String],
        reply: oneshot::Sender<Result<Option<String>, CallError>>,
    ) {
        if caller == module {
            let _ = reply.send(Err(CallError::Itself));
            return;
        }
        let answer = match self.deliver(module, action, args) {
            Ok(answer) => answer,
            Err(undelivered) => {
                let error = match undelivered {
                    Undelivered::UnknownModule => CallError::NotEnabled(module.to_owned()),
                    Undelivered::UnknownAction => CallError::UnknownAction {
                        module: module.to_owned(),
                        action: action.to_owned(),
                    },
                    Undelivered::InvalidArgs(message) => CallError::InvalidArgs(message),
                    Undelivered::NotRunning => {
                        CallError::Failed(format!("{module} is not running"))
                    }
                };
                let _ = reply.send(Err(error));
                return;
            }
        };
        tokio::spawn(async move {
            let result = match answer.await {
                Ok(result) => result.map_err(CallError::Failed),
                Err(_) => Err(CallError::Failed(
                    "the module dropped the command without answering".into(),
                )),
            };
            let _ = reply.send(result);
        });
    }

    /// Checks a command against the module's actions and hands it over. The
    /// receiver gets the module's answer.
    fn deliver(
        &self,
        module: &str,
        action: &str,
        args: &[String],
    ) -> Result<oneshot::Receiver<Reply>, Undelivered> {
        let slot = self.modules.get(module).ok_or(Undelivered::UnknownModule)?;
        let spec = slot
            .actions
            .iter()
            .find(|spec| spec.name == action)
            .ok_or(Undelivered::UnknownAction)?;
        let args = actions::parse(spec, args)
            .map_err(|error| Undelivered::InvalidArgs(error.to_string()))?;
        let (command, reply) = ModuleCommand::new(action.to_owned(), args);
        let delivered = slot
            .events
            .as_ref()
            .is_some_and(|events| events.send(ModuleEvent::Command(command)).is_ok());
        if delivered {
            Ok(reply)
        } else {
            Err(Undelivered::NotRunning)
        }
    }

    fn on_list_actions(&mut self, id: ConnectionId, module: Option<String>) {
        let wanted: Vec<&'static str> = match module {
            None => self.order.clone(),
            Some(module) => match self.order.iter().find(|id| **id == module) {
                Some(found) => vec![*found],
                None => {
                    let message = format!("no module {module:?} is enabled");
                    self.reply_error(id, ErrorCode::UnknownModule, message);
                    return;
                }
            },
        };
        let modules = wanted
            .into_iter()
            .map(|module| ModuleActions {
                module: module.to_owned(),
                actions: self.modules[module].actions.clone(),
            })
            .collect();
        self.reply(id, DaemonMessage::Actions { modules });
    }

    /// Reloads `theme.toml` and `config.toml`. A file with an error changes
    /// nothing: the daemon keeps running as it was.
    fn on_reload(&mut self, id: ConnectionId) {
        let loaded = match self.store.reload() {
            Ok(loaded) => loaded,
            Err(error) => {
                self.reply_error(id, ErrorCode::InvalidConfig, error.to_string());
                return;
            }
        };
        // Restarts what ended, and picks up plugins' new files, even when the
        // config is the same.
        if let Err(error) = self.apply(&loaded.config) {
            self.reply_error(id, ErrorCode::Internal, format!("{error:#}"));
            return;
        }
        self.config = loaded.config.clone();
        if let Err(error) = self.run_with(loaded) {
            self.reply_error(id, ErrorCode::Internal, format!("{error:#}"));
            return;
        }
        let modules = self.order.iter().map(|id| (*id).to_owned()).collect();
        self.broadcast(&DaemonMessage::Modules { modules });
        self.broadcast(&DaemonMessage::Contributions {
            contributions: self.contributions(),
        });
        tracing::info!(modules = ?self.order, "reloaded config.toml and theme.toml");
        self.reply(id, DaemonMessage::Ok);
    }

    /// The settings panel reading or changing the settings. A change applies
    /// at once, like a reload that only touches what changed.
    fn on_settings(&mut self, op: SettingsOp) -> Result<Value, String> {
        let op = match op {
            SettingsOp::Snapshot => return Ok(self.store.snapshot()),
            SettingsOp::Text { path } => return self.store.text(&path).map(Value::String),
            SettingsOp::Export { format } => return self.store.export(&format).map(Value::String),
            SettingsOp::Set { path, value } => settings::Op::Set { path, value },
            SettingsOp::Reset { path } => settings::Op::Reset { path },
            SettingsOp::Discard => settings::Op::Discard,
            SettingsOp::Edit { path, text } => settings::Op::Edit { path, text },
            SettingsOp::Preview { values, replace } => settings::Op::Preview { values, replace },
            SettingsOp::Keep => settings::Op::Keep,
            SettingsOp::Drop => settings::Op::Drop,
        };
        let loaded = self.store.change(&op)?;
        self.run_with(loaded)
            .map_err(|error| format!("{error:#}"))?;
        Ok(Value::Null)
    }

    /// Runs with a new config and theme: applies the config when it changed
    /// and sends the theme when it did, then tells the settings panel.
    fn run_with(&mut self, loaded: settings::Loaded) -> anyhow::Result<()> {
        if loaded.config != self.config {
            self.apply(&loaded.config)?;
            self.config = loaded.config;
            let modules = self.order.iter().map(|id| (*id).to_owned()).collect();
            self.broadcast(&DaemonMessage::Modules { modules });
            self.broadcast(&DaemonMessage::Contributions {
                contributions: self.contributions(),
            });
        }
        if loaded.theme != self.theme {
            self.theme = loaded.theme;
            self.broadcast(&DaemonMessage::Theme {
                theme: Box::new(self.theme.clone()),
            });
        }
        self.notify(SETTINGS, ModuleEvent::Settings(self.store.snapshot()));
        Ok(())
    }

    /// What the enabled modules offer the enabled ones: an offer to a
    /// module that isn't running, like a tour step without the tour, goes
    /// nowhere.
    fn contributions(&self) -> Vec<Contribution> {
        self.order
            .iter()
            .flat_map(|module| self.modules[module].contributions.clone())
            .filter(|offer| self.order.contains(&offer.target.as_str()))
            .collect()
    }

    fn on_request(&mut self, ModuleRequest { module, request }: ModuleRequest) {
        let now = Instant::now();
        let result = match request {
            Request::PublishState(state) => {
                self.tell_watchers(module, &state);
                self.states.insert(module.to_owned(), state.clone());
                let message = DaemonMessage::State {
                    module: module.to_owned(),
                    state,
                };
                self.broadcast(&message);
                Ok(())
            }
            Request::PublishLive(value) => {
                self.broadcast(&DaemonMessage::Live {
                    module: module.to_owned(),
                    value,
                });
                Ok(())
            }
            Request::Present { id, mut spec } => {
                let views = std::iter::once(&spec.compact).chain(spec.expanded.as_ref());
                if !self.has_views(module, views) {
                    return;
                }
                // Panels open where `[island] panels` says, and the rest
                // where `notices` does; the idle island stays everywhere.
                if spec.output.is_none() && spec.overlay.is_none() {
                    if spec.modal {
                        spec.output = self.panel_output();
                    } else if spec.priority > Priority::IDLE {
                        spec.output = self.notice_output();
                    }
                }
                self.arbiter.submit(id, module, spec, now);
                Ok(())
            }
            Request::Update { id, payload } => self
                .arbiter
                .update(module, id, payload)
                .map_err(|error| error.to_string()),
            Request::Withdraw { id } => self
                .arbiter
                .withdraw(module, id, now)
                .map_err(|error| error.to_string()),
            Request::ShowBubble { id, spec } => {
                if self.has_views(module, std::iter::once(&spec.view).chain(&spec.wide)) {
                    self.bubbles.show(id, module, spec);
                }
                return;
            }
            Request::UpdateBubble { id, payload } => self
                .bubbles
                .update(module, id, payload)
                .map_err(|error| error.to_string()),
            Request::HideBubble { id } => self
                .bubbles
                .hide(module, id)
                .map_err(|error| error.to_string()),
            Request::WatchState { module: watched } => {
                if let Some(state) = self.states.get(&watched) {
                    let event = ModuleEvent::State {
                        module: watched.clone(),
                        state: state.clone(),
                    };
                    self.notify(module, event);
                }
                self.watchers.entry(watched).or_default().insert(module);
                return;
            }
            Request::Call {
                module: target,
                action,
                args,
                reply,
            } => {
                self.on_call(module, &target, &action, &args, reply);
                return;
            }
            Request::PauseIsland(paused) => {
                let only = paused.then(|| module.to_owned());
                self.arbiter.set_exclusive(only.clone(), now);
                self.bubbles.set_only(only);
                return;
            }
            Request::Settings { op, reply } => {
                let _ = reply.send(self.on_settings(op));
                return;
            }
        };
        if let Err(error) = result {
            tracing::warn!(module, %error, "module request failed");
        }
    }

    /// Whether the module ships every view it names. A missing one is a bug
    /// in the module, logged instead of shown.
    fn has_views<'a>(&self, module: &str, views: impl IntoIterator<Item = &'a String>) -> bool {
        let assets = &self.modules[module].assets;
        match views.into_iter().find(|view| !assets.has_view(view)) {
            Some(missing) => {
                tracing::error!(module, view = %missing, "the module has no such view");
                false
            }
            None => true,
        }
    }

    fn on_exit(
        &mut self,
        module: &'static str,
        generation: u64,
        result: Result<Result<(), ModuleError>, JoinError>,
    ) {
        // A run that a reload already stopped or replaced.
        if self
            .modules
            .get(module)
            .is_none_or(|slot| slot.generation != generation)
        {
            if let Ok(Err(error)) = &result {
                tracing::warn!(module, %error, "a stopped module ended with an error");
            }
            return;
        }
        match result {
            Ok(Ok(())) => tracing::info!(module, "module stopped"),
            Ok(Err(error)) => {
                tracing::error!(module, %error, "module failed");
                self.failed.insert(module, error.to_string());
            }
            Err(error) => {
                tracing::error!(module, %error, "module panicked");
                self.failed.insert(module, error.to_string());
            }
        }
        if let Some(slot) = self.modules.get_mut(module) {
            slot.events = None;
        }
        self.arbiter.withdraw_all(module, Instant::now());
        self.bubbles.hide_all(module);
    }

    fn on_ui_process(&mut self, event: UiEvent) {
        match event {
            UiEvent::Started { .. } => {
                self.handshake_deadline = Some(Instant::now() + HANDSHAKE_TIMEOUT);
            }
            UiEvent::GaveUp => {
                self.handshake_deadline = None;
                tracing::error!("the UI is not running; fix the error above and restart mochid");
            }
        }
    }

    /// The monitor a panel opens on, from `[island] panels`; `None` for every
    /// monitor.
    fn panel_output(&self) -> Option<String> {
        let focused = || self.compositor.state().focused_output;
        match self.panels {
            Panels::All => None,
            Panels::Focus => focused(),
            Panels::Pointer => self.compositor.pointer_output().or_else(focused),
        }
    }

    /// The monitor everything else shows on, from `[island] notices`;
    /// `None` for every monitor.
    fn notice_output(&self) -> Option<String> {
        let focused = || self.compositor.state().focused_output;
        match self.notices {
            Notices::All => None,
            Notices::Focus => focused(),
            Notices::Pointer => self.compositor.pointer_output().or_else(focused),
        }
    }

    fn apply_effects(&mut self) {
        if self.bubbles.take_changed() {
            self.broadcast(&self.bubbles_message());
            self.update_hidden_list();
        }
        for effect in self.arbiter.take_effects() {
            match effect {
                Effect::Present(activity) => {
                    if let Some(activity) = &activity {
                        tracing::debug!(id = %activity.id, module = %activity.module, view = %activity.view, "present");
                    }
                    let resting = self.arbiter.resting();
                    self.broadcast(&DaemonMessage::Present { activity, resting });
                }
                Effect::Clicked { module, activity } => {
                    self.notify(&module, ModuleEvent::Clicked(activity));
                }
                Effect::Hovered {
                    module,
                    activity,
                    hovered,
                } => {
                    self.notify(&module, ModuleEvent::Hovered { activity, hovered });
                }
                Effect::Ended {
                    module,
                    activity,
                    reason,
                } => {
                    if self.hidden_list.is_some_and(|(list, _)| list == activity) {
                        self.hidden_list = None;
                    }
                    self.notify(&module, ModuleEvent::Ended { activity, reason });
                }
            }
        }
    }

    /// Sends a module's new state to the modules watching it.
    fn tell_watchers(&self, module: &str, state: &Value) {
        let Some(watchers) = self.watchers.get(module) else {
            return;
        };
        for watcher in watchers {
            let event = ModuleEvent::State {
                module: module.to_owned(),
                state: state.clone(),
            };
            self.notify(watcher, event);
        }
    }

    fn plugin_status(&self) -> Vec<PluginStatus> {
        self.listed
            .iter()
            .map(|listed| {
                let running = self.order.iter().find(|id| **id == listed.id);
                let failed = running.and_then(|id| self.failed.get(id));
                let (state, message) = match (&listed.problem, running, failed) {
                    (Some(problem), _, _) => (PluginState::Missing, Some(problem.clone())),
                    (None, Some(_), Some(error)) => (PluginState::Failed, Some(error.clone())),
                    (None, Some(_), None) => (PluginState::Running, None),
                    (None, None, _) => (
                        PluginState::Disabled,
                        Some("add it to `modules` in config.toml".into()),
                    ),
                };
                PluginStatus {
                    id: listed.id.clone(),
                    state,
                    message,
                }
            })
            .collect()
    }

    fn notify(&self, module: &str, event: ModuleEvent) {
        if let Some(events) = self
            .modules
            .get(module)
            .and_then(|slot| slot.events.as_ref())
        {
            let _ = events.send(event);
        }
    }

    fn bubbles_message(&self) -> DaemonMessage {
        let (bubbles, overflow) = self.bubbles.snapshot();
        DaemonMessage::Bubbles {
            bubbles,
            overflow,
            stack: self.bubbles.stack(),
        }
    }

    fn compositor_status(&self) -> CompositorStatus {
        let state = self.compositor.state();
        CompositorStatus {
            backend: state.backend.to_string(),
            outputs: state
                .outputs
                .into_iter()
                .map(|output| output.name)
                .collect(),
            workspaces: state.workspaces.len(),
            focused: state.focused_output,
        }
    }

    fn ui_connected(&self) -> bool {
        self.clients
            .values()
            .any(|client| client.role == Some(Role::Ui))
    }

    /// Sends to every UI connection.
    fn broadcast(&self, message: &DaemonMessage) {
        for client in self.clients.values() {
            if client.role == Some(Role::Ui) {
                let _ = client.sender.send(message.clone());
            }
        }
    }

    fn reply(&self, id: ConnectionId, message: DaemonMessage) {
        if let Some(client) = self.clients.get(&id) {
            let _ = client.sender.send(message);
        }
    }

    fn reply_error(&self, id: ConnectionId, code: ErrorCode, message: impl Into<String>) {
        let message = message.into();
        self.reply(id, DaemonMessage::Error { code, message });
    }
}

/// The files plugins put in place of builtin views, for the enabled
/// modules. When two plugins replace the same view, the one whose id
/// sorts first wins.
/// The core view that lists an area's hidden bubbles, `island/<view>.qml`.
const HIDDEN_VIEW: &str = "HiddenBubbles";

fn hidden_payload(area: Area, bubbles: &[mochi_protocol::Bubble]) -> Value {
    serde_json::json!({ "area": area, "bubbles": bubbles })
}

fn overrides(
    plugins: &BTreeMap<&'static str, crate::plugins::PluginModule>,
    enabled: &[&'static str],
    modules: &[Box<dyn Module>],
) -> BTreeMap<&'static str, Vec<(String, PathBuf)>> {
    let mut overrides: BTreeMap<&'static str, Vec<(String, PathBuf)>> = BTreeMap::new();
    let mut owners: BTreeMap<String, &'static str> = BTreeMap::new();
    for (id, plugin) in plugins {
        if !enabled.contains(id) {
            continue;
        }
        for (module, view, file) in plugin.overrides() {
            let name = format!("{module}/{view}");
            let Some(target) = modules
                .iter()
                .find(|candidate| candidate.id() == module && enabled.contains(&candidate.id()))
            else {
                tracing::debug!(plugin = id, view = %name, "replaces a view of a module that isn't enabled");
                continue;
            };
            if !target.assets().has_view(&view) {
                tracing::warn!(plugin = id, view = %name, "replaces a view that doesn't exist");
                continue;
            }
            if !file.is_file() {
                tracing::warn!(plugin = id, file = %file.display(), "the override's file is missing");
                continue;
            }
            if let Some(owner) = owners.get(&name) {
                tracing::warn!(plugin = id, view = %name, owner, "another plugin already replaces this view");
                continue;
            }
            owners.insert(name, id);
            overrides
                .entry(target.id())
                .or_default()
                .push((format!("{view}.qml"), file));
        }
    }
    overrides
}

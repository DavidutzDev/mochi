//! The interface every module implements, builtin or plugin.
//!
//! A module runs as its own task. It talks to the daemon only through its
//! [`ModuleCtx`]: it publishes state, activities and bubbles, and receives
//! commands and events about its activities and bubbles. Every `ModuleCtx` call maps to one protocol message, which
//! is what lets an external plugin process stand in for a builtin module.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use include_dir::Dir;
use mochi_compositor::Compositor;
use mochi_protocol::{ActionSpec, ActivityId, BubbleId};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::actions::Args;
use crate::arbiter::{ActivitySpec, EndReason};
use crate::bubbles::BubbleSpec;
use crate::contributions::ContributionSpec;
pub use mochi_protocol::spec::CallError;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub type ModuleError = Box<dyn std::error::Error + Send + Sync>;

pub trait Module: Send + 'static {
    /// Stable identifier: the name in `config.toml`, in `mochi ipc <module>`
    /// and in `modules/<id>/` for its views.
    fn id(&self) -> &'static str;

    /// The module's QML views.
    fn assets(&self) -> Assets;

    /// The actions `mochi ipc <module> <action>` can run.
    fn actions(&self) -> Vec<ActionSpec> {
        Vec::new()
    }

    /// Its commented `[module.<id>]` section for the generated `config.toml`
    /// and the documentation: usually `include_str!("../settings.toml")`.
    /// Defaults go on lines like `# timeout_ms = 1500`; check them with
    /// [`crate::examples::check_module`] in a test.
    fn settings_example(&self) -> &'static str {
        ""
    }

    /// Checks its `[module.<id>]` table the way `run` will read it, so a typo
    /// fails at startup or reload with the file and the key, not later.
    /// Usually `settings::<Settings>(table).map(drop)`.
    fn check_settings(&self, table: &toml::Table) -> Result<(), String> {
        match table.keys().next() {
            None => Ok(()),
            Some(key) => Err(format!("unknown setting `{key}`: this module has none")),
        }
    }

    /// What it offers other modules, like a page for the hub. Unused when
    /// the module it's for isn't enabled.
    fn contributions(&self) -> Vec<ContributionSpec> {
        Vec::new()
    }

    /// Runs the module until the daemon shuts down, which closes
    /// [`ModuleCtx::next_event`]. Returning early stops the module and
    /// withdraws its activities.
    fn run(self: Box<Self>, ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>>;
}

/// Reads a `[module.<id>]` table into a module's settings type.
pub fn settings<T: DeserializeOwned>(table: &toml::Table) -> Result<T, toml::de::Error> {
    toml::Value::Table(table.clone()).try_into()
}

/// A module's QML views: embedded in the binary and also known by their
/// path in the source tree, which dev mode links to for hot reload; or, for
/// a plugin, a directory on disk.
///
/// ```ignore
/// static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");
///
/// fn assets(&self) -> Assets {
///     Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
/// }
/// ```
#[derive(Debug, Clone)]
pub enum Assets {
    Embedded {
        dir: &'static Dir<'static>,
        source: &'static str,
    },
    /// Always linked, never copied, so edits hot-reload.
    Disk(PathBuf),
}

impl Assets {
    pub const fn new(embedded: &'static Dir<'static>, source: &'static str) -> Self {
        Self::Embedded {
            dir: embedded,
            source,
        }
    }

    /// Whether `<view>.qml` is there.
    pub fn has_view(&self, view: &str) -> bool {
        let file = format!("{view}.qml");
        match self {
            Self::Embedded { dir, .. } => dir.get_file(&file).is_some(),
            Self::Disk(dir) => dir.join(&file).is_file(),
        }
    }
}

/// Hands out activity and bubble ids that are unique across all modules.
#[derive(Debug, Clone, Default)]
pub struct ActivityIds(Arc<AtomicU64>);

impl ActivityIds {
    pub fn next(&self) -> ActivityId {
        ActivityId(self.0.fetch_add(1, Ordering::Relaxed) + 1)
    }
}

/// What a module asks the daemon to do.
#[derive(Debug)]
pub struct ModuleRequest {
    pub module: &'static str,
    pub request: Request,
}

#[derive(Debug)]
pub enum Request {
    PublishState(Value),
    Present {
        id: ActivityId,
        spec: ActivitySpec,
    },
    Update {
        id: ActivityId,
        payload: Value,
    },
    Withdraw {
        id: ActivityId,
    },
    ShowBubble {
        id: BubbleId,
        spec: BubbleSpec,
    },
    UpdateBubble {
        id: BubbleId,
        payload: Value,
    },
    HideBubble {
        id: BubbleId,
    },
    /// Sends it another module's state from now on.
    WatchState {
        module: String,
    },
    /// Runs another module's action.
    Call {
        module: String,
        action: String,
        args: Vec<String>,
        reply: oneshot::Sender<Result<(), CallError>>,
    },
}

/// What the daemon tells a module.
#[derive(Debug)]
pub enum ModuleEvent {
    Command(ModuleCommand),
    /// A click on one of its activities that has no expanded view.
    Clicked(ActivityId),
    /// One of its activities is gone for good.
    Ended {
        activity: ActivityId,
        reason: EndReason,
    },
    /// A click on one of its bubbles.
    BubbleClicked(BubbleId),
    /// A module it watches published state: see [`ModuleCtx::watch_state`].
    /// `Value::Null` when that module stopped.
    State {
        module: String,
        state: Value,
    },
}

/// What a module answers a command with: some output to hand back, or none.
pub type Reply = Result<Option<String>, String>;

/// A validated `mochi ipc` command. Answer it with [`ModuleCommand::reply`],
/// or [`ModuleCommand::answer`] to hand something back. A module may hold it
/// until the user decides; dropping it unanswered reports a failure to the
/// caller.
#[derive(Debug)]
pub struct ModuleCommand {
    pub action: String,
    pub args: Args,
    reply: oneshot::Sender<Reply>,
}

impl ModuleCommand {
    /// Returns the command and the receiver its reply arrives on.
    pub fn new(action: String, args: Args) -> (Self, oneshot::Receiver<Reply>) {
        let (reply, receiver) = oneshot::channel();
        (
            Self {
                action,
                args,
                reply,
            },
            receiver,
        )
    }

    pub fn reply(self, result: Result<(), String>) {
        self.send(result.map(|()| None));
    }

    /// Replies with output, which `mochi ipc` prints.
    pub fn answer(self, result: Result<String, String>) {
        self.send(result.map(Some));
    }

    fn send(self, reply: Reply) {
        // The caller may have disconnected; there is nobody left to tell.
        let _ = self.reply.send(reply);
    }
}

#[derive(Debug)]
pub struct ModuleCtx {
    module: &'static str,
    settings: toml::Table,
    compositor: Compositor,
    ids: ActivityIds,
    data_dir: PathBuf,
    session_dir: PathBuf,
    requests: mpsc::UnboundedSender<ModuleRequest>,
    events: mpsc::UnboundedReceiver<ModuleEvent>,
}

impl ModuleCtx {
    /// Creates the context for one module. The daemon keeps the returned
    /// sender to deliver the module's events.
    pub fn new(
        module: &'static str,
        settings: toml::Table,
        compositor: Compositor,
        ids: ActivityIds,
        data_dir: PathBuf,
        session_dir: PathBuf,
        requests: mpsc::UnboundedSender<ModuleRequest>,
    ) -> (Self, mpsc::UnboundedSender<ModuleEvent>) {
        let (sender, events) = mpsc::unbounded_channel();
        let ctx = Self {
            module,
            settings,
            compositor,
            ids,
            data_dir,
            session_dir,
            requests,
            events,
        };
        (ctx, sender)
    }

    pub fn module(&self) -> &'static str {
        self.module
    }

    /// The module's `[module.<id>]` table from `config.toml`. Fields missing
    /// there take their `Default` values when `T` uses `#[serde(default)]`.
    pub fn settings<T: DeserializeOwned>(&self) -> Result<T, toml::de::Error> {
        settings(&self.settings)
    }

    /// Workspaces and outputs, the same for every compositor. Check
    /// `state().backend` before relying on it: without a supported compositor
    /// the state stays empty.
    pub fn compositor(&self) -> &Compositor {
        &self.compositor
    }

    /// A directory only this module writes to, emptied when the daemon
    /// starts. For files views load, like images a module received as
    /// bytes. It lives in the runtime directory, so it never outlives the
    /// session.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// A directory only this module writes to that, unlike
    /// [`data_dir`](Self::data_dir), keeps its files when the daemon
    /// restarts. It lives in the runtime directory too, so they still go at
    /// logout. Created on first use.
    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    /// Replaces the module's state, which the UI can read from any view.
    pub fn publish_state(&self, state: Value) {
        self.send(Request::PublishState(state));
    }

    /// Submits an activity to the arbiter and returns its id at once. Whether
    /// and when it shows is up to the arbiter.
    pub fn present(&self, spec: ActivitySpec) -> ActivityId {
        let id = self.ids.next();
        self.send(Request::Present { id, spec });
        id
    }

    /// Replaces the payload of one of the module's activities.
    pub fn update(&self, id: ActivityId, payload: Value) {
        self.send(Request::Update { id, payload });
    }

    /// Removes one of the module's activities, shown or waiting.
    pub fn withdraw(&self, id: ActivityId) {
        self.send(Request::Withdraw { id });
    }

    /// Shows a bubble, or replaces the module's bubble with the same key in
    /// place. Returns its id at once.
    pub fn show_bubble(&self, spec: BubbleSpec) -> BubbleId {
        let id = BubbleId(self.ids.next().0);
        self.send(Request::ShowBubble { id, spec });
        id
    }

    /// Replaces the payload of one of the module's bubbles.
    pub fn update_bubble(&self, id: BubbleId, payload: Value) {
        self.send(Request::UpdateBubble { id, payload });
    }

    pub fn hide_bubble(&self, id: BubbleId) {
        self.send(Request::HideBubble { id });
    }

    /// Runs another module's action, with the same checks as `mochi ipc`.
    /// Fails with [`CallError::NotEnabled`] when that module isn't enabled,
    /// so a module can use another one when it's there and carry on when it
    /// isn't.
    ///
    /// The future doesn't borrow the context. Spawn it rather than awaiting
    /// it inside the event loop when the other module might call back: each
    /// would wait for the other.
    pub fn call(
        &self,
        module: &str,
        action: &str,
        args: &[&str],
    ) -> impl Future<Output = Result<(), CallError>> + Send + 'static {
        let (reply, answer) = oneshot::channel();
        self.send(Request::Call {
            module: module.to_owned(),
            action: action.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            reply,
        });
        async move {
            answer
                .await
                .unwrap_or_else(|_| Err(CallError::Failed("the daemon stopped".into())))
        }
    }

    /// Sends it `module`'s state as [`ModuleEvent::State`]: the latest at
    /// once, when there is one, then every change. Fine to call before that
    /// module starts, or when it isn't enabled.
    pub fn watch_state(&self, module: &str) {
        self.send(Request::WatchState {
            module: module.to_owned(),
        });
    }

    /// The next command or activity event. `None` once the daemon is
    /// shutting down.
    pub async fn next_event(&mut self) -> Option<ModuleEvent> {
        self.events.recv().await
    }

    fn send(&self, request: Request) {
        // Fails only while the daemon shuts down, when the request no longer
        // matters.
        let _ = self.requests.send(ModuleRequest {
            module: self.module,
            request,
        });
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    fn context(settings: &str) -> (ModuleCtx, mpsc::UnboundedReceiver<ModuleRequest>) {
        let (requests, received) = mpsc::unbounded_channel();
        let settings = toml::from_str(settings).unwrap();
        let (ctx, _events) = ModuleCtx::new(
            "clock",
            settings,
            Compositor::unsupported(),
            ActivityIds::default(),
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent"),
            requests,
        );
        (ctx, received)
    }

    #[test]
    fn ids_are_unique_across_clones() {
        let ids = ActivityIds::default();
        let other = ids.clone();
        assert_eq!(ids.next(), ActivityId(1));
        assert_eq!(other.next(), ActivityId(2));
    }

    #[test]
    fn requests_are_tagged_with_the_module() {
        let (ctx, mut received) = context("");
        ctx.publish_state(json!({ "time": "12:00" }));
        let id = ctx.present(ActivitySpec::new("Pill"));
        ctx.withdraw(id);

        let first = received.try_recv().unwrap();
        assert_eq!(first.module, "clock");
        assert!(matches!(first.request, Request::PublishState(_)));
        assert!(
            matches!(received.try_recv().unwrap().request, Request::Present { id: sent, .. } if sent == id)
        );
        assert!(matches!(
            received.try_recv().unwrap().request,
            Request::Withdraw { .. }
        ));
    }

    #[test]
    fn settings_deserialize_with_defaults() {
        #[derive(Debug, Deserialize, Default, PartialEq)]
        #[serde(default, deny_unknown_fields)]
        struct Settings {
            format: String,
            seconds: bool,
        }

        let (ctx, _) = context("format = \"HH:mm\"");
        assert_eq!(
            ctx.settings::<Settings>().unwrap(),
            Settings {
                format: "HH:mm".into(),
                seconds: false
            }
        );

        let (ctx, _) = context("fromat = \"HH:mm\"");
        assert!(ctx.settings::<Settings>().is_err());
    }

    #[tokio::test]
    async fn commands_reach_the_module_and_replies_come_back() {
        let (requests, _received) = mpsc::unbounded_channel();
        let (mut ctx, events) = ModuleCtx::new(
            "clock",
            toml::Table::new(),
            Compositor::unsupported(),
            ActivityIds::default(),
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent"),
            requests,
        );

        let (command, reply) = ModuleCommand::new("show".into(), Args::default());
        events.send(ModuleEvent::Command(command)).unwrap();

        let Some(ModuleEvent::Command(command)) = ctx.next_event().await else {
            panic!("expected a command");
        };
        assert_eq!(command.action, "show");
        command.reply(Err("nothing to show".into()));
        assert_eq!(reply.await.unwrap(), Err("nothing to show".to_owned()));
    }

    #[tokio::test]
    async fn dropping_a_command_is_visible_to_the_caller() {
        let (command, reply) = ModuleCommand::new("show".into(), Args::default());
        drop(command);
        assert!(reply.await.is_err());
    }

    #[tokio::test]
    async fn events_end_when_the_daemon_drops_its_sender() {
        let (requests, _received) = mpsc::unbounded_channel();
        let (mut ctx, events) = ModuleCtx::new(
            "clock",
            toml::Table::new(),
            Compositor::unsupported(),
            ActivityIds::default(),
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent"),
            requests,
        );
        drop(events);
        assert!(ctx.next_event().await.is_none());
    }
}

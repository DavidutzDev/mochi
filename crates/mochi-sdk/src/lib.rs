//! Write Mochi plugin backends in Rust.
//!
//! A plugin backend is a program mochid starts and talks to over a socket.
//! This crate does the talking: [`ModuleCtx`] has the same calls as a
//! builtin module's context in mochid, so a plugin can show activities on
//! the island, bubbles, answer `mochi ipc` actions and publish state for
//! its views, like any builtin.
//!
//! ```no_run
//! use mochi_sdk::{ActivitySpec, ModuleCtx, ModuleEvent, json};
//!
//! fn main() -> std::process::ExitCode {
//!     mochi_sdk::run(hello)
//! }
//!
//! async fn hello(mut ctx: ModuleCtx) -> Result<(), mochi_sdk::Error> {
//!     while let Some(event) = ctx.next_event().await {
//!         if let ModuleEvent::Command(command) = event {
//!             // `mochi ipc hello say`, declared in mochi-plugin.toml.
//!             let spec = ActivitySpec::new("Hello").payload(json!({ "text": "Hi" }));
//!             ctx.present(spec);
//!             command.reply(Ok(()));
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! The backend runs until mochid closes the connection: then
//! [`ModuleCtx::next_event`] returns `None`, and the plugin should return.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::future::Future;
use std::os::fd::FromRawFd;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The protocol's message types, for tests that play mochid's part: see
/// [`ModuleCtx::over`].
pub use mochi_protocol as protocol;
pub use mochi_protocol::plugin::{CompositorState, OutputInfo, WindowInfo, WorkspaceInfo};
pub use mochi_protocol::spec::{
    ActivitySpec, ArgValue, Args, BubbleSpec, CallError, EndReason, Priority, SamePriority,
};
pub use mochi_protocol::{API, ActivityId, Area, BubbleId, Contribution};
pub use serde_json::{Value, json};

use mochi_protocol::plugin::{FD_ENV, FromPlugin, ToPlugin};
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot, watch};

/// Any error a plugin returns: `?` turns most errors into it, and
/// `"text".into()` makes one from a message.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Connects to mochid, runs `plugin` until it returns, and turns its result
/// into an exit code. Runs on a single-threaded tokio runtime; to pick
/// another, use [`ModuleCtx::connect`] from your own.
pub fn run<F, Fut>(plugin: F) -> ExitCode
where
    F: FnOnce(ModuleCtx) -> Fut,
    Fut: Future<Output = Result<(), Error>>,
{
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            report(&format!("cannot start the async runtime: {error}"));
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async {
        let ctx = ModuleCtx::connect().await?;
        plugin(ctx).await
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error.to_string());
            ExitCode::FAILURE
        }
    }
}

/// mochid logs what a backend prints on stderr.
#[allow(clippy::print_stderr)]
fn report(message: &str) {
    eprintln!("{message}");
}

/// What mochid tells the plugin.
#[derive(Debug)]
pub enum ModuleEvent {
    /// `mochi ipc <plugin> <action>`, or a call from another module. Answer
    /// it with [`ModuleCommand::reply`] or [`ModuleCommand::answer`].
    Command(ModuleCommand),
    /// A click on one of its activities that has no expanded view.
    Clicked(ActivityId),
    /// One of its activities is gone for good.
    Ended {
        /// The id [`ModuleCtx::present`] returned.
        activity: ActivityId,
        /// Why: it timed out, the user closed it, the plugin withdrew it,
        /// another with its key replaced it, or a click outside closed it.
        reason: EndReason,
    },
    /// A click on one of its bubbles.
    BubbleClicked(BubbleId),
    /// A module from `[uses] state` in the manifest published state:
    /// `Value::Null` when it stopped. Sent once at the start when it has
    /// some, then on every change.
    State {
        /// The module's id.
        module: String,
        /// Its whole state, as it published it.
        state: Value,
    },
    /// What the enabled modules offer this plugin: every contribution whose
    /// `target` is its id. Sent at the start, then when a reload changes it.
    Offers(Vec<Contribution>),
}

/// An action to run, with arguments already checked against the manifest.
/// Dropping it without an answer tells the caller it failed.
#[derive(Debug)]
pub struct ModuleCommand {
    /// The action's name, from the manifest.
    pub action: String,
    /// Its arguments, parsed into the kinds the manifest declares: read
    /// them with [`Args::str`], [`Args::int`], [`Args::float`] and
    /// [`Args::bool`]. Optional ones that were left out are missing.
    pub args: Args,
    id: u64,
    outgoing: Option<mpsc::UnboundedSender<FromPlugin>>,
}

impl ModuleCommand {
    /// Answers: `Ok(())` for success, `Err` with a message `mochi ipc`
    /// prints as the failure.
    pub fn reply(mut self, result: Result<(), String>) {
        self.send(None, result.err());
    }

    /// Replies with output, which `mochi ipc` prints.
    pub fn answer(mut self, result: Result<String, String>) {
        match result {
            Ok(output) => self.send(Some(output), None),
            Err(error) => self.send(None, Some(error)),
        }
    }

    fn send(&mut self, output: Option<String>, error: Option<String>) {
        if let Some(outgoing) = self.outgoing.take() {
            let _ = outgoing.send(FromPlugin::Reply {
                id: self.id,
                output,
                error,
            });
        }
    }
}

impl Drop for ModuleCommand {
    fn drop(&mut self) {
        self.send(
            None,
            Some("the plugin dropped the command without answering".into()),
        );
    }
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<ToPlugin>>>>;

/// The plugin's connection to mochid. Every call is one message, and none
/// waits for mochid unless it returns a future.
#[derive(Debug)]
pub struct ModuleCtx {
    module: String,
    version: String,
    settings: Value,
    data_dir: PathBuf,
    session_dir: PathBuf,
    ids: AtomicU64,
    requests: AtomicU64,
    outgoing: mpsc::UnboundedSender<FromPlugin>,
    events: mpsc::UnboundedReceiver<ModuleEvent>,
    pending: Pending,
    compositor: watch::Receiver<CompositorState>,
}

impl ModuleCtx {
    /// Connects through the socket mochid passes in `MOCHI_PLUGIN_FD` and
    /// says hello. Fails when the program wasn't started by mochid.
    pub async fn connect() -> Result<Self, Error> {
        let fd: i32 = std::env::var(FD_ENV)
            .map_err(|_| format!("{FD_ENV} isn't set: mochid starts plugin backends, not you"))?
            .parse()
            .map_err(|_| format!("{FD_ENV} isn't a file descriptor"))?;
        static TAKEN: AtomicBool = AtomicBool::new(false);
        if TAKEN.swap(true, Ordering::SeqCst) {
            return Err("already connected to mochid".into());
        }
        // SAFETY: mochid opens this descriptor for the backend and nothing
        // else in the process owns it; `TAKEN` makes sure it's wrapped once.
        let stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(fd) };
        stream.set_nonblocking(true)?;
        Self::over(UnixStream::from_std(stream)?).await
    }

    /// Talks to mochid over `stream`: for tests, which play mochid's part
    /// on the other end. mochid speaks first, with `hello`.
    ///
    /// ```
    /// use mochi_sdk::protocol::plugin::{CompositorState, FromPlugin, ToPlugin};
    /// use mochi_sdk::{ActivitySpec, ModuleCtx, json};
    /// use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    /// use tokio::net::UnixStream;
    ///
    /// # tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
    /// let (mochid, plugin) = UnixStream::pair().unwrap();
    /// let (reader, mut writer) = mochid.into_split();
    /// let hello = ToPlugin::Hello {
    ///     api: mochi_sdk::API,
    ///     version: "test".into(),
    ///     module: "timer".into(),
    ///     settings: json!({ "minutes": 5 }),
    ///     data_dir: "/tmp/timer".into(),
    ///     session_dir: "/tmp/timer".into(),
    ///     compositor: CompositorState::default(),
    /// };
    /// writer.write_all(&mochi_sdk::protocol::encode(&hello).unwrap()).await.unwrap();
    ///
    /// let ctx = ModuleCtx::over(plugin).await.unwrap();
    /// ctx.present(ActivitySpec::new("Timer"));
    ///
    /// // What the plugin sent: its hello, then the activity.
    /// let mut lines = BufReader::new(reader).lines();
    /// let mut next = async || {
    ///     let line = lines.next_line().await.unwrap().unwrap();
    ///     mochi_sdk::protocol::decode::<FromPlugin>(&line).unwrap()
    /// };
    /// assert_eq!(next().await, FromPlugin::Hello { api: mochi_sdk::API });
    /// assert!(matches!(next().await, FromPlugin::Present { id: 1, .. }));
    /// # });
    /// ```
    pub async fn over(stream: UnixStream) -> Result<Self, Error> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let ToPlugin::Hello {
            api,
            version,
            module,
            settings,
            data_dir,
            session_dir,
            compositor,
        } = mochi_protocol::decode(&line)?
        else {
            return Err("mochid didn't start with hello".into());
        };
        if api != API {
            return Err(format!("mochid speaks api {api}, this plugin speaks {API}").into());
        }
        writer
            .write_all(&mochi_protocol::encode(&FromPlugin::Hello { api: API })?)
            .await?;

        let (outgoing, mut to_write) = mpsc::unbounded_channel::<FromPlugin>();
        tokio::spawn(async move {
            while let Some(message) = to_write.recv().await {
                let Ok(line) = mochi_protocol::encode(&message) else {
                    continue;
                };
                if writer.write_all(&line).await.is_err() {
                    break;
                }
            }
        });

        let (events, received) = mpsc::unbounded_channel();
        let (compositor_sender, compositor) = watch::channel(compositor);
        let pending = Pending::default();
        tokio::spawn(read(
            reader,
            events,
            compositor_sender,
            pending.clone(),
            outgoing.clone(),
        ));

        Ok(Self {
            module,
            version,
            settings,
            data_dir,
            session_dir,
            ids: AtomicU64::new(0),
            requests: AtomicU64::new(0),
            outgoing,
            events: received,
            pending,
            compositor,
        })
    }

    /// The plugin's id.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// mochid's version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The plugin's `[module.<id>]` table from config.toml. Fields missing
    /// there take their `Default` values when `T` uses `#[serde(default)]`.
    pub fn settings<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        let settings = match &self.settings {
            Value::Null => Value::Object(serde_json::Map::new()),
            other => other.clone(),
        };
        serde_json::from_value(settings)
    }

    /// A directory only this plugin writes to, emptied when mochid starts,
    /// in the runtime directory: for files its views load.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Like [`data_dir`](Self::data_dir), but kept across mochid restarts.
    /// Created on first use.
    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    /// Replaces the plugin's state, which its views read as
    /// `Daemon.state("<id>")` and other modules can watch.
    pub fn publish_state(&self, state: Value) {
        self.send(FromPlugin::PublishState { state });
    }

    /// Submits an activity to the island. Whether and when it shows is up
    /// to the arbiter.
    pub fn present(&self, spec: ActivitySpec) -> ActivityId {
        let id = self.next_id();
        self.send(FromPlugin::Present { id, spec });
        ActivityId(id)
    }

    /// Replaces an activity's payload. Its view updates in place.
    pub fn update(&self, id: ActivityId, payload: Value) {
        self.send(FromPlugin::Update { id: id.0, payload });
    }

    /// Removes an activity, shown or waiting. It ends with
    /// [`EndReason::Withdrawn`].
    pub fn withdraw(&self, id: ActivityId) {
        self.send(FromPlugin::Withdraw { id: id.0 });
    }

    /// Shows a bubble, or replaces the plugin's bubble with the same key.
    pub fn show_bubble(&self, spec: BubbleSpec) -> BubbleId {
        let id = self.next_id();
        self.send(FromPlugin::ShowBubble { id, spec });
        BubbleId(id)
    }

    /// Replaces a bubble's payload, keeping its place. Not news: see
    /// [`BubbleSpec::news`].
    pub fn update_bubble(&self, id: BubbleId, payload: Value) {
        self.send(FromPlugin::UpdateBubble { id: id.0, payload });
    }

    /// Removes a bubble.
    pub fn hide_bubble(&self, id: BubbleId) {
        self.send(FromPlugin::HideBubble { id: id.0 });
    }

    /// Runs another module's action, with the same checks as `mochi ipc`.
    /// [`CallError::NotEnabled`] means that module isn't enabled.
    pub fn call(
        &self,
        module: &str,
        action: &str,
        args: &[&str],
    ) -> impl Future<Output = Result<(), CallError>> + Send + 'static {
        let answer = self.ask(module, action, args);
        async move { answer.await.map(drop) }
    }

    /// Like [`ModuleCtx::call`], and returns what the action answered with,
    /// like the output `mochi ipc` prints.
    pub fn ask(
        &self,
        module: &str,
        action: &str,
        args: &[&str],
    ) -> impl Future<Output = Result<Option<String>, CallError>> + Send + 'static {
        let (id, answer) = self.request();
        self.send(FromPlugin::Call {
            id,
            module: module.to_owned(),
            action: action.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        });
        async move {
            match answer.await {
                Ok(ToPlugin::CallResult {
                    error: None,
                    output,
                    ..
                }) => Ok(output),
                Ok(ToPlugin::CallResult {
                    error: Some(error), ..
                }) => Err(error),
                _ => Err(CallError::Failed("mochid stopped".into())),
            }
        }
    }

    /// The compositor's state as it is now.
    pub fn compositor(&self) -> CompositorState {
        self.compositor.borrow().clone()
    }

    /// Wakes on every new compositor state.
    pub fn compositor_changes(&self) -> watch::Receiver<CompositorState> {
        self.compositor.clone()
    }

    /// Switches to a workspace, by its id from [`CompositorState`].
    pub fn activate_workspace(
        &self,
        workspace: u32,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        let (id, answer) = self.request();
        self.send(FromPlugin::ActivateWorkspace { id, workspace });
        async move { compositor_answer(answer).await.map(drop) }
    }

    /// The windows on visible workspaces, where the compositor says.
    pub fn windows(
        &self,
    ) -> impl Future<Output = Result<Vec<WindowInfo>, String>> + Send + 'static {
        let (id, answer) = self.request();
        self.send(FromPlugin::Windows { id });
        async move {
            let value = compositor_answer(answer).await?;
            serde_json::from_value(value).map_err(|error| error.to_string())
        }
    }

    /// The output the pointer is on, when the compositor says.
    pub fn pointer_output(
        &self,
    ) -> impl Future<Output = Result<Option<String>, String>> + Send + 'static {
        let (id, answer) = self.request();
        self.send(FromPlugin::PointerOutput { id });
        async move {
            let value = compositor_answer(answer).await?;
            Ok(value.as_str().map(str::to_owned))
        }
    }

    /// The next command or event. `None` once mochid closes the
    /// connection: the plugin should finish then.
    pub async fn next_event(&mut self) -> Option<ModuleEvent> {
        self.events.recv().await
    }

    fn next_id(&self) -> u64 {
        self.ids.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn request(&self) -> (u64, oneshot::Receiver<ToPlugin>) {
        let id = self.requests.fetch_add(1, Ordering::Relaxed) + 1;
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, sender);
        (id, receiver)
    }

    fn send(&self, message: FromPlugin) {
        // Fails only once mochid is gone, when nothing matters anymore.
        let _ = self.outgoing.send(message);
    }
}

async fn compositor_answer(answer: oneshot::Receiver<ToPlugin>) -> Result<Value, String> {
    match answer.await {
        Ok(ToPlugin::CompositorResult {
            error: None, value, ..
        }) => Ok(value),
        Ok(ToPlugin::CompositorResult {
            error: Some(error), ..
        }) => Err(error),
        _ => Err("mochid stopped".into()),
    }
}

/// Reads mochid's messages until it closes the connection.
async fn read(
    mut reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    events: mpsc::UnboundedSender<ModuleEvent>,
    compositor: watch::Sender<CompositorState>,
    pending: Pending,
    outgoing: mpsc::UnboundedSender<FromPlugin>,
) {
    let mut line = Vec::new();
    loop {
        line.clear();
        let limit = mochi_protocol::MAX_LINE as u64 + 1;
        match (&mut reader).take(limit).read_until(b'\n', &mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) if line.len() > mochi_protocol::MAX_LINE => break,
            Ok(_) => {}
        }
        let Ok(message) = std::str::from_utf8(&line)
            .map_err(|error| error.to_string())
            .and_then(|text| {
                mochi_protocol::decode::<ToPlugin>(text).map_err(|error| error.to_string())
            })
        else {
            // A newer mochid may send messages this SDK doesn't know.
            continue;
        };
        let event = match message {
            ToPlugin::Command { id, action, args } => ModuleEvent::Command(ModuleCommand {
                action,
                args,
                id,
                outgoing: Some(outgoing.clone()),
            }),
            ToPlugin::Clicked { activity } => ModuleEvent::Clicked(ActivityId(activity)),
            ToPlugin::Ended { activity, reason } => ModuleEvent::Ended {
                activity: ActivityId(activity),
                reason,
            },
            ToPlugin::BubbleClicked { bubble } => ModuleEvent::BubbleClicked(BubbleId(bubble)),
            ToPlugin::State { module, state } => ModuleEvent::State { module, state },
            ToPlugin::Offers { offers } => ModuleEvent::Offers(offers),
            ToPlugin::Compositor { state } => {
                compositor.send_replace(state);
                continue;
            }
            answer @ (ToPlugin::CallResult { id, .. } | ToPlugin::CompositorResult { id, .. }) => {
                let waiting = pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&id);
                if let Some(waiting) = waiting {
                    let _ = waiting.send(answer);
                }
                continue;
            }
            ToPlugin::Hello { .. } => continue,
        };
        if events.send(event).is_err() {
            break;
        }
    }
    // Waiting calls fail now that mochid is gone.
    pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mochi_protocol::spec::ArgValue;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    use super::*;

    /// Plays mochid: says hello and hands back both ends of the talk.
    async fn mochid() -> (
        ModuleCtx,
        BufReader<tokio::net::unix::OwnedReadHalf>,
        tokio::net::unix::OwnedWriteHalf,
    ) {
        let (ours, theirs) = UnixStream::pair().unwrap();
        let (reader, mut writer) = ours.into_split();
        let hello = ToPlugin::Hello {
            api: API,
            version: "test".into(),
            module: "timer".into(),
            settings: json!({ "minutes": 5 }),
            data_dir: PathBuf::from("/tmp/data"),
            session_dir: PathBuf::from("/tmp/session"),
            compositor: CompositorState::default(),
        };
        writer
            .write_all(&mochi_protocol::encode(&hello).unwrap())
            .await
            .unwrap();
        let ctx = ModuleCtx::over(theirs).await.unwrap();
        let mut reader = BufReader::new(reader);
        assert_eq!(receive(&mut reader).await, FromPlugin::Hello { api: API });
        (ctx, reader, writer)
    }

    async fn receive(reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>) -> FromPlugin {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        mochi_protocol::decode(&line).unwrap()
    }

    async fn send(writer: &mut tokio::net::unix::OwnedWriteHalf, message: ToPlugin) {
        writer
            .write_all(&mochi_protocol::encode(&message).unwrap())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn settings_and_requests() {
        #[derive(serde::Deserialize, Default)]
        #[serde(default)]
        struct Settings {
            minutes: u32,
            label: String,
        }
        let (ctx, mut reader, _writer) = mochid().await;
        let settings: Settings = ctx.settings().unwrap();
        assert_eq!(settings.minutes, 5);
        assert_eq!(settings.label, "");
        assert_eq!(ctx.module(), "timer");

        let first = ctx.present(ActivitySpec::new("Timer"));
        let bubble = ctx.show_bubble(BubbleSpec::new("Dot"));
        assert_eq!(first, ActivityId(1));
        assert_eq!(bubble, BubbleId(2));
        assert!(matches!(
            receive(&mut reader).await,
            FromPlugin::Present { id: 1, .. }
        ));
        assert!(matches!(
            receive(&mut reader).await,
            FromPlugin::ShowBubble { id: 2, .. }
        ));
    }

    #[tokio::test]
    async fn commands_are_answered_or_fail_when_dropped() {
        let (mut ctx, mut reader, mut writer) = mochid().await;
        let args: Args = [("minutes".to_owned(), ArgValue::Int(3))]
            .into_iter()
            .collect();
        send(
            &mut writer,
            ToPlugin::Command {
                id: 7,
                action: "start".into(),
                args,
            },
        )
        .await;
        send(
            &mut writer,
            ToPlugin::Command {
                id: 8,
                action: "stop".into(),
                args: Args::default(),
            },
        )
        .await;

        let Some(ModuleEvent::Command(command)) = ctx.next_event().await else {
            panic!("no command");
        };
        assert_eq!(command.args.int("minutes"), Some(3));
        command.answer(Ok("started".into()));
        let Some(ModuleEvent::Command(command)) = ctx.next_event().await else {
            panic!("no command");
        };
        drop(command);

        assert_eq!(
            receive(&mut reader).await,
            FromPlugin::Reply {
                id: 7,
                output: Some("started".into()),
                error: None
            }
        );
        assert!(matches!(
            receive(&mut reader).await,
            FromPlugin::Reply {
                id: 8,
                error: Some(_),
                ..
            }
        ));
    }

    #[tokio::test]
    async fn calls_get_their_answers() {
        let (ctx, mut reader, mut writer) = mochid().await;
        let call = ctx.call("media", "toggle", &[]);
        let FromPlugin::Call { id, module, .. } = receive(&mut reader).await else {
            panic!("no call");
        };
        assert_eq!(module, "media");
        send(
            &mut writer,
            ToPlugin::CallResult {
                id,
                error: Some(CallError::NotEnabled("media".into())),
                output: None,
            },
        )
        .await;
        assert_eq!(call.await, Err(CallError::NotEnabled("media".into())));

        let ask = ctx.ask("launcher", "search", &["2+2"]);
        let FromPlugin::Call { id, .. } = receive(&mut reader).await else {
            panic!("no call");
        };
        send(
            &mut writer,
            ToPlugin::CallResult {
                id,
                error: None,
                output: Some("4".into()),
            },
        )
        .await;
        assert_eq!(ask.await, Ok(Some("4".into())));

        let pointer = ctx.pointer_output();
        let FromPlugin::PointerOutput { id } = receive(&mut reader).await else {
            panic!("no request");
        };
        send(
            &mut writer,
            ToPlugin::CompositorResult {
                id,
                value: json!("DP-3"),
                error: None,
            },
        )
        .await;
        assert_eq!(pointer.await, Ok(Some("DP-3".into())));
    }

    #[tokio::test]
    async fn the_end_of_the_connection_ends_events() {
        let (mut ctx, reader, writer) = mochid().await;
        let call = ctx.call("media", "toggle", &[]);
        drop((reader, writer));
        assert!(ctx.next_event().await.is_none());
        assert!(call.await.is_err());
    }
}

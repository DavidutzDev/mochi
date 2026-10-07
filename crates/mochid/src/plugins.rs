//! Plugins as modules. A [`PluginModule`] is a module like any builtin:
//! its views come from the plugin's directory, its actions and
//! contributions from its manifest, and its `run` starts the backend and
//! carries messages both ways between it and the [`ModuleCtx`], so the
//! backend can do everything a builtin can.
//!
//! The backend is supervised: after a crash it starts again, a little later
//! each time, and after [`CRASHES`] crashes within [`CRASH_WINDOW`] it
//! stays stopped until the next reload, with a desktop notification saying
//! so.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, UNIX_EPOCH};

use mochi_core::compositor::{self, Compositor, WorkspaceId};
use mochi_core::{
    ActivityId, Assets, BoxFuture, BubbleId, ContributionSpec, Module, ModuleCommand, ModuleCtx,
    ModuleError, ModuleEvent,
};
use mochi_plugins::Manifest;
use mochi_protocol::plugin::{
    CompositorState, DIR_ENV, FD_ENV, FromPlugin, OutputInfo, ToPlugin, WindowInfo, WorkspaceInfo,
};
use mochi_protocol::{API, ActionSpec, MAX_LINE};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// This many crashes within [`CRASH_WINDOW`] stop a backend for good.
const CRASHES: usize = 5;
const CRASH_WINDOW: Duration = Duration::from_secs(60);
/// The first restart's delay, doubled after each crash up to
/// [`LONGEST_DELAY`].
const FIRST_DELAY: Duration = Duration::from_millis(250);
const LONGEST_DELAY: Duration = Duration::from_secs(8);
/// How long a new backend gets to answer `hello`.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a backend gets to exit once mochid closes its socket.
const STOP_TIMEOUT: Duration = Duration::from_millis(1500);
/// The backend's end of the socket.
const BACKEND_FD: i32 = 3;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Module ids live as long as the daemon. Plugin ids come from files, so
/// each distinct one is leaked once and reused after.
pub fn intern(id: &str) -> &'static str {
    static IDS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut ids = IDS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(known) = ids.iter().find(|known| **known == id) {
        return known;
    }
    let leaked: &'static str = Box::leak(id.to_owned().into_boxed_str());
    ids.push(leaked);
    leaked
}

#[derive(Debug, Clone)]
pub struct PluginModule {
    id: &'static str,
    dir: PathBuf,
    manifest: Manifest,
    /// Its settings at their defaults, from the `# key = value` lines of its
    /// `settings.toml`. `None` without one, which leaves them unchecked.
    defaults: Option<mochi_core::toml::Table>,
}

impl PluginModule {
    pub fn new(dir: PathBuf, manifest: Manifest) -> Self {
        let id = intern(&manifest.plugin.id);
        let defaults = std::fs::read_to_string(dir.join("settings.toml"))
            .ok()
            .and_then(|example| {
                let parsed: Result<mochi_core::toml::Table, _> =
                    mochi_core::toml::from_str(&mochi_core::examples::uncommented(&example));
                match parsed {
                    Ok(table) => Some(table),
                    Err(error) => {
                        tracing::warn!(
                            plugin = id,
                            "its settings.toml doesn't parse, so its settings go unchecked: {error}"
                        );
                        None
                    }
                }
            })
            .map(|table| {
                table
                    .get("module")
                    .and_then(|modules| modules.get(id))
                    .and_then(|section| section.as_table())
                    .cloned()
                    .unwrap_or_default()
            });
        Self {
            id,
            dir,
            manifest,
            defaults,
        }
    }

    pub fn views_dir(&self) -> PathBuf {
        self.dir.join(&self.manifest.views.dir)
    }

    /// The builtin views it replaces: (module, view, its file).
    pub fn overrides(&self) -> Vec<(String, String, PathBuf)> {
        self.manifest
            .overrides()
            .into_iter()
            .map(|(module, view)| {
                let file = self
                    .views_dir()
                    .join("overrides")
                    .join(&module)
                    .join(format!("{view}.qml"));
                (module, view, file)
            })
            .collect()
    }

    /// Changes when the plugin's files do: its manifest or its backend.
    /// A reload restarts a plugin whose revision changed.
    pub fn revision(&self) -> String {
        let stamp = |path: &Path| {
            std::fs::metadata(path)
                .ok()
                .map(|metadata| {
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .unwrap_or_default();
                    format!(
                        "{}.{}:{}",
                        modified.as_secs(),
                        modified.subsec_nanos(),
                        metadata.len()
                    )
                })
                .unwrap_or_default()
        };
        let mut revision = stamp(&self.dir.join(mochi_plugins::manifest::FILE));
        if let Some(backend) = &self.manifest.backend {
            revision.push('/');
            revision.push_str(&stamp(&self.dir.join(&backend.exec)));
        }
        revision
    }
}

impl Module for PluginModule {
    fn id(&self) -> &'static str {
        self.id
    }

    fn assets(&self) -> Assets {
        Assets::Disk(self.views_dir())
    }

    fn actions(&self) -> Vec<ActionSpec> {
        self.manifest.actions()
    }

    /// Checks the settings against the defaults in its `settings.toml`:
    /// every key must be one of them, with a value of the same type.
    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        match &self.defaults {
            Some(defaults) => check_against(defaults, table, ""),
            None => Ok(()),
        }
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        self.manifest
            .contributions()
            .into_iter()
            .map(|offer| {
                let mut spec = ContributionSpec::new(
                    offer.target,
                    offer.kind,
                    offer.id,
                    offer.view,
                    offer.title,
                )
                .order(offer.order)
                .options(offer.options);
                if let Some(icon) = offer.icon {
                    spec = spec.icon(icon);
                }
                spec
            })
            .collect()
    }

    fn run(self: Box<Self>, ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(supervise(*self, ctx))
    }
}

/// How one run of the backend ended.
enum Ended {
    /// mochid is stopping the plugin.
    Stopped,
    /// The backend exited.
    Exited(ExitStatus),
    /// It never started, or broke the protocol.
    Failed(String),
}

async fn supervise(plugin: PluginModule, mut ctx: ModuleCtx) -> Result<(), ModuleError> {
    let mut link = Link::default();
    for module in &plugin.manifest.uses.state {
        ctx.watch_state(module);
    }
    let Some(backend) = plugin.manifest.backend.clone() else {
        // Views only: nothing to run, but events still come.
        while let Some(event) = ctx.next_event().await {
            if let ModuleEvent::Command(command) = event {
                command.reply(Err(format!("{} has no backend to run actions", plugin.id)));
            }
        }
        return Ok(());
    };

    let mut crashes: VecDeque<Instant> = VecDeque::new();
    let mut delay = FIRST_DELAY;
    loop {
        let ended = session(&plugin, &backend, &mut ctx, &mut link).await;
        // Stopping, the daemon already took away what the plugin showed.
        if matches!(ended, Ended::Stopped) {
            return Ok(());
        }
        link.clear(&ctx);
        let reason = match ended {
            Ended::Stopped => return Ok(()),
            Ended::Exited(status) if status.success() => {
                tracing::info!(plugin = plugin.id, "the backend exited");
                return Ok(());
            }
            Ended::Exited(status) => format!("the backend {status}"),
            Ended::Failed(reason) => reason,
        };
        let now = Instant::now();
        crashes.push_back(now);
        while crashes
            .front()
            .is_some_and(|first| now.duration_since(*first) > CRASH_WINDOW)
        {
            crashes.pop_front();
        }
        if crashes.len() >= CRASHES {
            let message = format!(
                "{reason}; it crashed {CRASHES} times within a minute, so it stays stopped until `mochi reload`"
            );
            tokio::spawn(notify_failure(
                plugin.manifest.plugin.name.clone(),
                message.clone(),
            ));
            return Err(message.into());
        }
        tracing::warn!(plugin = plugin.id, %reason, ?delay, "restarting the backend");

        // Wait, answering commands that come meanwhile.
        let restart = tokio::time::sleep(delay);
        tokio::pin!(restart);
        loop {
            tokio::select! {
                () = &mut restart => break,
                event = ctx.next_event() => match event {
                    None => return Ok(()),
                    Some(ModuleEvent::Command(command)) => {
                        command.reply(Err(format!("{} is restarting", plugin.id)));
                    }
                    Some(ModuleEvent::State { module, state }) => {
                        link.states.insert(module, state);
                    }
                    Some(ModuleEvent::Offers(offers)) => link.offers = Some(offers),
                    Some(_) => {}
                },
            }
        }
        delay = (delay * 2).min(LONGEST_DELAY);
    }
}

/// What the adapter keeps across messages: the backend's numbers for its
/// activities and bubbles against the daemon's, and the commands it hasn't
/// answered yet.
#[derive(Default)]
struct Link {
    activities: HashMap<u64, ActivityId>,
    bubbles: HashMap<u64, BubbleId>,
    commands: HashMap<u64, ModuleCommand>,
    next_command: u64,
    /// The latest state of each watched module, for the next backend.
    states: BTreeMap<String, Value>,
    /// What's offered to the plugin, for the next backend.
    offers: Option<Vec<mochi_protocol::Contribution>>,
}

impl Link {
    /// After a backend ends, what it showed goes with it, and its pending
    /// commands fail.
    fn clear(&mut self, ctx: &ModuleCtx) {
        for (_, id) in self.activities.drain() {
            ctx.withdraw(id);
        }
        for (_, id) in self.bubbles.drain() {
            ctx.hide_bubble(id);
        }
        for (_, command) in self.commands.drain() {
            command.reply(Err("the plugin's backend stopped".into()));
        }
    }

    fn activity(&self, id: ActivityId) -> Option<u64> {
        self.activities
            .iter()
            .find(|(_, theirs)| **theirs == id)
            .map(|(ours, _)| *ours)
    }

    fn bubble(&self, id: BubbleId) -> Option<u64> {
        self.bubbles
            .iter()
            .find(|(_, theirs)| **theirs == id)
            .map(|(ours, _)| *ours)
    }
}

async fn session(
    plugin: &PluginModule,
    backend: &mochi_plugins::manifest::Backend,
    ctx: &mut ModuleCtx,
    link: &mut Link,
) -> Ended {
    let (stream, mut child) = match start(plugin, backend) {
        Ok(started) => started,
        Err(error) => return Ended::Failed(format!("cannot start the backend: {error}")),
    };
    let id = plugin.id;
    forward_output(id, &mut child);
    let (mut incoming, outgoing) = connect(id, stream);

    let settings: mochi_core::toml::Table = ctx.settings().unwrap_or_default();
    let mut compositor = ctx.compositor().subscribe();
    let hello = ToPlugin::Hello {
        api: API,
        version: VERSION.into(),
        module: id.into(),
        settings: serde_json::to_value(settings).unwrap_or_default(),
        data_dir: ctx.data_dir().to_owned(),
        session_dir: ctx.session_dir().to_owned(),
        compositor: wire_state(&compositor.borrow_and_update()),
    };
    let _ = outgoing.send(hello);

    match tokio::time::timeout(HELLO_TIMEOUT, incoming.recv()).await {
        Ok(Some(Ok(FromPlugin::Hello { api }))) if api == API => {}
        Ok(Some(Ok(FromPlugin::Hello { api }))) => {
            stop(child).await;
            return Ended::Failed(format!("the backend speaks api {api}, mochid speaks {API}"));
        }
        Ok(Some(Ok(other))) => {
            stop(child).await;
            return Ended::Failed(format!("the backend sent {other:?} before hello"));
        }
        Ok(Some(Err(error))) => {
            stop(child).await;
            return Ended::Failed(error);
        }
        Ok(None) => return exited(child).await,
        Err(_) => {
            stop(child).await;
            return Ended::Failed(format!(
                "the backend didn't say hello within {} s",
                HELLO_TIMEOUT.as_secs()
            ));
        }
    }
    tracing::info!(plugin = id, "the backend started");
    for (module, state) in &link.states {
        let _ = outgoing.send(ToPlugin::State {
            module: module.clone(),
            state: state.clone(),
        });
    }
    if let Some(offers) = &link.offers {
        let _ = outgoing.send(ToPlugin::Offers {
            offers: offers.clone(),
        });
    }

    loop {
        tokio::select! {
            message = incoming.recv() => match message {
                Some(Ok(message)) => on_message(id, message, ctx, link, &outgoing),
                Some(Err(error)) => tracing::warn!(plugin = id, %error, "the backend sent a malformed message"),
                None => return exited(child).await,
            },
            event = ctx.next_event() => match event {
                None => {
                    drop(outgoing);
                    stop(child).await;
                    return Ended::Stopped;
                }
                Some(event) => on_event(event, link, &outgoing),
            },
            changed = compositor.changed() => {
                if changed.is_ok() {
                    let state = wire_state(&compositor.borrow_and_update());
                    let _ = outgoing.send(ToPlugin::Compositor { state });
                }
            }
            status = child.wait() => {
                return match status {
                    Ok(status) => Ended::Exited(status),
                    Err(error) => Ended::Failed(error.to_string()),
                };
            }
        }
    }
}

fn on_message(
    id: &'static str,
    message: FromPlugin,
    ctx: &ModuleCtx,
    link: &mut Link,
    outgoing: &mpsc::UnboundedSender<ToPlugin>,
) {
    let unknown = |what: &str, number: u64| {
        tracing::warn!(
            plugin = id,
            "the backend named {what} {number}, which it doesn't have"
        );
    };
    match message {
        FromPlugin::Hello { .. } => tracing::warn!(plugin = id, "the backend said hello twice"),
        FromPlugin::PublishState { state } => ctx.publish_state(state),
        FromPlugin::Present { id: ours, spec } => {
            if let Some(old) = link.activities.insert(ours, ctx.present(spec)) {
                ctx.withdraw(old);
            }
        }
        FromPlugin::Update { id: ours, payload } => match link.activities.get(&ours) {
            Some(theirs) => ctx.update(*theirs, payload),
            None => unknown("activity", ours),
        },
        FromPlugin::Withdraw { id: ours } => match link.activities.remove(&ours) {
            Some(theirs) => ctx.withdraw(theirs),
            None => unknown("activity", ours),
        },
        FromPlugin::ShowBubble { id: ours, spec } => {
            let theirs = ctx.show_bubble(spec);
            if let Some(old) = link.bubbles.insert(ours, theirs) {
                ctx.hide_bubble(old);
            }
        }
        FromPlugin::UpdateBubble { id: ours, payload } => match link.bubbles.get(&ours) {
            Some(theirs) => ctx.update_bubble(*theirs, payload),
            None => unknown("bubble", ours),
        },
        FromPlugin::HideBubble { id: ours } => match link.bubbles.remove(&ours) {
            Some(theirs) => ctx.hide_bubble(theirs),
            None => unknown("bubble", ours),
        },
        FromPlugin::Reply {
            id: number,
            output,
            error,
        } => match link.commands.remove(&number) {
            Some(command) => match (output, error) {
                (_, Some(error)) => command.reply(Err(error)),
                (Some(output), None) => command.answer(Ok(output)),
                (None, None) => command.reply(Ok(())),
            },
            None => unknown("command", number),
        },
        FromPlugin::Call {
            id: number,
            module,
            action,
            args,
        } => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let call = ctx.ask(&module, &action, &args);
            let outgoing = outgoing.clone();
            tokio::spawn(async move {
                let (output, error) = match call.await {
                    Ok(output) => (output, None),
                    Err(error) => (None, Some(error)),
                };
                let _ = outgoing.send(ToPlugin::CallResult {
                    id: number,
                    error,
                    output,
                });
            });
        }
        FromPlugin::ActivateWorkspace {
            id: number,
            workspace,
        } => {
            let result = ctx
                .compositor()
                .activate_workspace(WorkspaceId(workspace))
                .map(|()| Value::Null)
                .map_err(|error| error.to_string());
            answer(outgoing, number, result);
        }
        FromPlugin::Windows { id: number } => {
            let compositor = ctx.compositor().clone();
            let outgoing = outgoing.clone();
            tokio::spawn(async move {
                let result = windows(&compositor).await;
                answer(&outgoing, number, result);
            });
        }
        FromPlugin::PointerOutput { id: number } => {
            let output = ctx.compositor().pointer_output();
            answer(
                outgoing,
                number,
                Ok(output.map(Value::from).unwrap_or_default()),
            );
        }
    }
}

fn on_event(event: ModuleEvent, link: &mut Link, outgoing: &mpsc::UnboundedSender<ToPlugin>) {
    let message = match event {
        ModuleEvent::Command(command) => {
            link.next_command += 1;
            let id = link.next_command;
            let message = ToPlugin::Command {
                id,
                action: command.action.clone(),
                args: command.args.clone(),
            };
            link.commands.insert(id, command);
            message
        }
        ModuleEvent::Clicked(activity) => match link.activity(activity) {
            Some(ours) => ToPlugin::Clicked { activity: ours },
            None => return,
        },
        ModuleEvent::Hovered { activity, hovered } => match link.activity(activity) {
            Some(ours) => ToPlugin::Hovered {
                activity: ours,
                hovered,
            },
            None => return,
        },
        ModuleEvent::Ended { activity, reason } => match link.activity(activity) {
            Some(ours) => {
                link.activities.remove(&ours);
                ToPlugin::Ended {
                    activity: ours,
                    reason,
                }
            }
            None => return,
        },
        ModuleEvent::BubbleClicked(bubble) => match link.bubble(bubble) {
            Some(ours) => ToPlugin::BubbleClicked { bubble: ours },
            None => return,
        },
        ModuleEvent::State { module, state } => {
            link.states.insert(module.clone(), state.clone());
            ToPlugin::State { module, state }
        }
        ModuleEvent::Offers(offers) => {
            link.offers = Some(offers.clone());
            ToPlugin::Offers { offers }
        }
    };
    let _ = outgoing.send(message);
}

fn answer(outgoing: &mpsc::UnboundedSender<ToPlugin>, id: u64, result: Result<Value, String>) {
    let (value, error) = match result {
        Ok(value) => (value, None),
        Err(error) => (Value::Null, Some(error)),
    };
    let _ = outgoing.send(ToPlugin::CompositorResult { id, value, error });
}

async fn windows(compositor: &Compositor) -> Result<Value, String> {
    let windows = compositor
        .windows()
        .await
        .map_err(|error| error.to_string())?;
    let windows: Vec<WindowInfo> = windows
        .into_iter()
        .map(|window| WindowInfo {
            title: window.title,
            app_id: window.app_id,
            x: window.x,
            y: window.y,
            width: window.width,
            height: window.height,
            floating: window.floating,
        })
        .collect();
    serde_json::to_value(windows).map_err(|error| error.to_string())
}

fn wire_state(state: &compositor::State) -> CompositorState {
    CompositorState {
        backend: state.backend.to_string(),
        outputs: state
            .outputs
            .iter()
            .map(|output| OutputInfo {
                name: output.name.clone(),
                description: output.description.clone(),
                width: output.width,
                height: output.height,
            })
            .collect(),
        workspaces: state
            .workspaces
            .iter()
            .map(|workspace| WorkspaceInfo {
                id: workspace.id.0,
                name: workspace.name.clone(),
                output: workspace.output.clone(),
                coordinates: workspace.coordinates.clone(),
                active: workspace.active,
                urgent: workspace.urgent,
                hidden: workspace.hidden,
                can_activate: workspace.can_activate,
            })
            .collect(),
        focused_output: state.focused_output.clone(),
        focused_app: state.focused_app.clone(),
        screencast: state.screencast,
        captured: state.captured.clone(),
    }
}

/// Starts the backend with its end of a socket pair as fd 3.
fn start(
    plugin: &PluginModule,
    backend: &mochi_plugins::manifest::Backend,
) -> io::Result<(UnixStream, Child)> {
    // A backend Nix built has what it needs on its own PATH, also when the
    // plugin's directory links to it.
    let built_by_nix = std::fs::canonicalize(plugin.dir.join(&backend.exec))
        .is_ok_and(|path| path.starts_with("/nix/store"));
    let missing: Vec<&str> = backend
        .needs
        .iter()
        .filter(|_| !built_by_nix)
        .filter(|command| !mochi_plugins::on_path(command))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        tracing::warn!(
            plugin = %plugin.id,
            missing = %missing.join(", "),
            "the plugin needs programs that aren't installed"
        );
    }
    let (ours, theirs) = std::os::unix::net::UnixStream::pair()?;
    let theirs = OwnedFd::from(theirs);
    let fd = theirs.as_raw_fd();
    let mut command = Command::new(plugin.dir.join(&backend.exec));
    command
        .args(&backend.args)
        .current_dir(&plugin.dir)
        .env(FD_ENV, BACKEND_FD.to_string())
        .env(DIR_ENV, &plugin.dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // SAFETY: the closure runs in the forked child before exec, where only
    // async-signal-safe calls are allowed: dup2, fcntl and prctl are. It
    // reads `fd`, a plain integer copied in, and touches no memory of the
    // parent's.
    unsafe {
        command.pre_exec(move || {
            if fd == BACKEND_FD {
                // dup2 onto itself keeps close-on-exec: clear it instead.
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
            } else if libc::dup2(fd, BACKEND_FD) == -1 {
                return Err(io::Error::last_os_error());
            }
            // The backend goes when mochid does, even killed.
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn()?;
    drop(theirs);
    ours.set_nonblocking(true)?;
    Ok((UnixStream::from_std(ours)?, child))
}

/// Logs what the backend prints, one line at a time.
fn forward_output(id: &'static str, child: &mut Child) {
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(plugin = id, "{line}");
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!(plugin = id, "{line}");
            }
        });
    }
}

type Incoming = mpsc::UnboundedReceiver<Result<FromPlugin, String>>;

/// Reads and writes the socket on tasks of their own.
fn connect(id: &'static str, stream: UnixStream) -> (Incoming, mpsc::UnboundedSender<ToPlugin>) {
    let (reader, mut writer) = stream.into_split();
    let (outgoing, mut to_write) = mpsc::unbounded_channel::<ToPlugin>();
    tokio::spawn(async move {
        while let Some(message) = to_write.recv().await {
            let Ok(line) = mochi_protocol::encode(&message) else {
                continue;
            };
            if writer.write_all(&line).await.is_err() {
                break;
            }
        }
        // Closing our end tells the backend to stop.
        let _ = writer.shutdown().await;
    });

    let (incoming, received) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        loop {
            line.clear();
            let limit = MAX_LINE as u64 + 1;
            match (&mut reader).take(limit).read_until(b'\n', &mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() > MAX_LINE => {
                    tracing::warn!(
                        plugin = id,
                        "the backend sent a line longer than {MAX_LINE} bytes"
                    );
                    break;
                }
                Ok(_) => {}
            }
            let message = std::str::from_utf8(&line)
                .map_err(|error| error.to_string())
                .and_then(|text| mochi_protocol::decode(text).map_err(|error| error.to_string()));
            if incoming.send(message).is_err() {
                break;
            }
        }
    });
    (received, outgoing)
}

/// The socket closed: the backend is exiting, or has.
async fn exited(mut child: Child) -> Ended {
    match tokio::time::timeout(STOP_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => Ended::Exited(status),
        Ok(Err(error)) => Ended::Failed(error.to_string()),
        Err(_) => {
            stop(child).await;
            Ended::Failed("the backend closed its socket but kept running".into())
        }
    }
}

/// Gives the backend a moment to exit on its own, then kills it.
async fn stop(mut child: Child) {
    if tokio::time::timeout(STOP_TIMEOUT, child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}

/// A desktop notification: shown by Mochi's notifications module when it
/// runs, or by whatever notification daemon does.
async fn notify_failure(name: String, message: String) {
    let result = async {
        let connection = zbus::Connection::session().await?;
        let hints: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
        connection
            .call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "Notify",
                &(
                    "Mochi",
                    0u32,
                    "dialog-warning",
                    format!("The {name} plugin stopped"),
                    message,
                    Vec::<&str>::new(),
                    hints,
                    -1i32,
                ),
            )
            .await?;
        Ok::<(), zbus::Error>(())
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, "could not send a notification about the plugin");
    }
}

/// Checks `table` against `defaults`, the way serde would for a struct with
/// these fields: no unknown keys, and each value of its default's type. An
/// integer passes for a float. `prefix` names the enclosing tables.
fn check_against(
    defaults: &mochi_core::toml::Table,
    table: &mochi_core::toml::Table,
    prefix: &str,
) -> Result<(), String> {
    use mochi_core::toml::Value;
    for (key, value) in table {
        let name = format!("{prefix}{key}");
        let Some(default) = defaults.get(key) else {
            let mut known: Vec<&str> = defaults.keys().map(String::as_str).collect();
            known.sort_unstable();
            return Err(if known.is_empty() {
                format!("unknown setting `{name}`")
            } else {
                format!(
                    "unknown setting `{name}`, expected one of `{}`",
                    known.join("`, `")
                )
            });
        };
        match (default, value) {
            // An empty table by default is a map of anything.
            (Value::Table(defaults), Value::Table(table)) if !defaults.is_empty() => {
                check_against(defaults, table, &format!("{name}."))?;
            }
            (Value::Float(_), Value::Integer(_)) => {}
            _ if std::mem::discriminant(default) == std::mem::discriminant(value) => {}
            _ => {
                return Err(format!(
                    "`{name}` should be {}, like the default {default}, not {}",
                    default.type_str(),
                    value.type_str()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(name: &str) -> PluginModule {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins")
            .join(name);
        let manifest = Manifest::load(&dir).unwrap();
        PluginModule::new(dir, manifest)
    }

    fn table(text: &str) -> mochi_core::toml::Table {
        mochi_core::toml::from_str(text).unwrap()
    }

    #[test]
    fn settings_are_checked_against_the_plugins_settings_toml() {
        let weather = example("weather");
        weather
            .check_settings(&table(
                "city = \"Lyon\"\nlatitude = 45\nrefresh_minutes = 5",
            ))
            .unwrap();

        let typo = weather
            .check_settings(&table("ctiy = \"Lyon\""))
            .unwrap_err();
        assert!(typo.contains("unknown setting `ctiy`"), "{typo}");
        assert!(typo.contains("`city`"), "{typo}");

        let wrong = weather
            .check_settings(&table("refresh_minutes = \"often\""))
            .unwrap_err();
        assert!(
            wrong.contains("`refresh_minutes` should be integer"),
            "{wrong}"
        );

        // Every example's own defaults pass.
        for name in ["weather", "pomodoro"] {
            let plugin = example(name);
            let defaults = plugin.defaults.clone().unwrap();
            assert!(!defaults.is_empty(), "{name} shows no defaults");
            plugin.check_settings(&defaults).unwrap();
        }
    }

    #[test]
    fn nested_tables_are_checked_and_empty_ones_take_anything() {
        let defaults = table("[colors]\nfocus = \"red\"\n[aliases]");
        check_against(&defaults, &table("[colors]\nfocus = \"blue\""), "").unwrap();
        check_against(&defaults, &table("[aliases]\nanything = 1"), "").unwrap();
        let error = check_against(&defaults, &table("[colors]\nbreak = \"x\""), "").unwrap_err();
        assert!(error.contains("`colors.break`"), "{error}");
    }

    #[test]
    fn interned_ids_are_shared() {
        let first = intern("pomodoro-test");
        let second = intern(&String::from("pomodoro-test"));
        assert!(std::ptr::eq(first, second));
    }
}

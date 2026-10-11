//! Lock: Mochi's lock screen. A small program of its own, `mochi-lock`,
//! takes the compositor's session lock and checks the password with PAM;
//! this module only decides when it runs.
//!
//! It locks on `mochi ipc lock lock`, when logind asks (`loginctl
//! lock-session`, the power module's Lock, an idle daemon), and before the
//! computer sleeps, holding the sleep back until the compositor confirms
//! the lock. Nothing here can unlock: only `mochi-lock` can, once PAM
//! accepts the password, and logind's Unlock signal is ignored.
//!
//! If `mochi-lock` dies before unlocking, the compositor keeps the screen
//! locked and this starts it again. It writes a marker with its process id
//! once locked and removes it once unlocked, so a restarted module, or a
//! restarted mochid, finds a lock that's still on and watches it, or starts
//! a new one when its process is gone.

mod logind;
mod screen;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Assets, BoxFuture, ContributionSpec, Module, ModuleCtx,
    ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Lock;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// The PAM service that checks the password, a file in /etc/pam.d.
    #[schemars(extend("x-source" = "pam-service"))]
    pam_service: String,
    /// Lock before the computer sleeps.
    before_sleep: bool,
    /// Lock when logind asks.
    logind: bool,
    /// Behind the lock screen.
    background: Background,
    /// The image behind it, blurred, when `background` is "image".
    #[schemars(extend("x-source" = "file", "x-filter" = "image"))]
    image: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pam_service: "mochi".into(),
            before_sleep: true,
            logind: true,
            background: Background::Wallpaper,
            image: String::new(),
        }
    }
}

/// What's behind the lock screen.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Background {
    /// The wallpaper awww, swww or hyprpaper shows, blurred.
    #[default]
    Wallpaper,
    /// The image `image` names, blurred.
    Image,
    /// The theme's background color.
    None,
    /// Each monitor as it was just before locking, blurred: what was open
    /// shows, blurred.
    Screenshot,
}

impl Settings {
    /// The image to blur behind the lock screen, if any: the wallpaper's
    /// (`auto`), the one `image` names, or none.
    fn backdrop(&self) -> Option<String> {
        match self.background {
            Background::Wallpaper => Some("auto".into()),
            Background::Image if !self.image.trim().is_empty() => {
                Some(self.image.trim().to_owned())
            }
            Background::Image | Background::None | Background::Screenshot => None,
        }
    }
}

/// The commands the lock screen may run, as in `mochi-lock`'s renderer.
const ALLOWED: [(&str, &str); 3] = [
    ("media", "play-pause"),
    ("media", "next"),
    ("media", "previous"),
];

/// How long the computer waits to sleep for the lock to show, at most.
const SLEEP_WAIT: Duration = Duration::from_secs(2);
/// How often a lock this module didn't start is checked on.
const WATCH: Duration = Duration::from_millis(500);

impl Module for Lock {
    fn id(&self) -> &'static str {
        "lock"
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
        // The password field: on the lock screen only, and always there.
        vec![
            ContributionSpec::new("widgets", "widget", "field", "Field", "Password")
                .icon("lock")
                .options(json!({
                    "places": ["lock"],
                    "required": true,
                    "frame": false,
                    "size": [24, 6],
                    "min": [16, 5],
                    "max": [40, 9],
                    "category": "Lock screen",
                    "description": "The password field, with what went wrong and Caps Lock",
                })),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("lock", "Lock the screen"),
            ActionSpec::new("edit", "Arrange the lock screen's widgets, or stop"),
            ActionSpec::new("status", "Say whether the screen is locked"),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let dir = ctx.session_dir().to_owned();
            let mut locker = Locker::new(&dir, &settings, &ctx);
            // The layout, and through it, the modules its widgets are from.
            ctx.watch_state("widgets");
            // The wallpaper blurred, off the loop: a lock doesn't wait on it.
            let blurring = {
                let (background, out) = (
                    settings.backdrop(),
                    ctx.data_dir().join("lock-background.png"),
                );
                tokio::task::spawn_blocking(move || screen::blur(background, &out))
            };
            tokio::pin!(blurring);
            let mut blurred = false;
            let (events, mut received) = mpsc::unbounded_channel();

            let system = zbus::Connection::system().await.ok();
            let manager = match &system {
                Some(system) => logind::manager(system).await.ok(),
                None => None,
            };
            let session = match &system {
                Some(system) => logind::session(system).await.ok(),
                None => None,
            };
            locker.session = session.clone();
            let mut lock_requests = match (&session, settings.logind) {
                (Some(session), true) => session.receive_lock().await.ok(),
                _ => None,
            };
            let mut sleeps = match (&manager, settings.before_sleep) {
                (Some(manager), true) => manager.receive_prepare_for_sleep().await.ok(),
                _ => None,
            };
            let mut inhibitor = None;
            if settings.before_sleep
                && let Some(manager) = &manager
            {
                inhibitor = inhibit(manager).await;
            }
            // When the sleep that waits for the lock may go on anyway.
            let mut sleep_deadline: Option<Instant> = None;

            // A lock left from before: watch it, or take over from it.
            locker.resume(&events).await;
            locker.publish(&ctx);

            let mut watch = tokio::time::interval(WATCH);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        // Stopping leaves a lock running: it unlocks only
                        // with the password, and the next start finds it.
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => match command.action.as_str() {
                            "lock" => {
                                let started = locker.lock(&events);
                                command.reply(started);
                            }
                            "status" => command.answer(Ok(locker.status().into())),
                            "edit" => {
                                let call = ctx.call("widgets", "edit-lock", &["toggle"]);
                                tokio::spawn(async move {
                                    let _ = call.await;
                                });
                                command.reply(Ok(()));
                            }
                            other => {
                                let error = format!("lock has no action {other}");
                                command.reply(Err(error));
                            }
                        },
                        Some(ModuleEvent::Clicked(id)) if locker.notice == Some(id) => {
                            ctx.withdraw(id);
                            locker.notice = None;
                        }
                        Some(ModuleEvent::Ended { activity, .. }) if locker.notice == Some(activity) => {
                            locker.notice = None;
                        }
                        Some(ModuleEvent::State { module, state }) => locker.relay(&ctx, &module, state),
                        Some(_) => {}
                    },
                    Some(event) = received.recv() => locker.child_event(event, &events, &ctx).await,
                    Some(_) = next(&mut lock_requests) => {
                        tracing::info!("logind asked to lock");
                        let _ = locker.lock(&events);
                    }
                    Some(signal) = next(&mut sleeps) => {
                        let start = signal.args().is_ok_and(|args| args.start);
                        if start {
                            tracing::info!("locking before sleep");
                            let _ = locker.lock(&events);
                            sleep_deadline = Some(Instant::now() + SLEEP_WAIT);
                        } else if inhibitor.is_none()
                            && let Some(manager) = &manager
                        {
                            inhibitor = inhibit(manager).await;
                        }
                    }
                    _ = watch.tick() => locker.check_adopted(&events, &ctx).await,
                    () = locker.wait_to_respawn() => locker.respawn(&events),
                    background = &mut blurring, if !blurred => {
                        blurred = true;
                        locker.background = background.ok().flatten();
                        locker.write_config(&settings, &ctx);
                    }
                }
                // The sleep goes on once the screen is locked, or the wait
                // is over.
                if let Some(deadline) = sleep_deadline
                    && (locker.secured || Instant::now() >= deadline)
                {
                    if !locker.secured {
                        tracing::warn!("going to sleep before the lock was confirmed");
                    }
                    inhibitor = None;
                    sleep_deadline = None;
                }
                locker.publish(&ctx);
            }
        })
    }
}

async fn next<S: futures_util::Stream + Unpin>(stream: &mut Option<S>) -> Option<S::Item> {
    match stream {
        Some(stream) => stream.next().await,
        None => std::future::pending().await,
    }
}

async fn inhibit(manager: &logind::ManagerProxy<'static>) -> Option<zbus::zvariant::OwnedFd> {
    match manager
        .inhibit("sleep", "Mochi", "Locking the screen first", "delay")
        .await
    {
        Ok(fd) => Some(fd),
        Err(error) => {
            tracing::warn!(%error, "can't hold sleep back to lock first");
            None
        }
    }
}

/// What a running `mochi-lock` said, or that it ended.
#[derive(Debug)]
enum ChildEvent {
    Line(u32, String),
    Exit(u32, Option<i32>),
}

/// What to do after `mochi-lock` ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Next {
    /// Unlocked, or refused for good: nothing more.
    Done,
    /// Start it again after this long: the screen is locked, or was asked
    /// to be, and needs a lock screen.
    Respawn(Duration),
}

/// The exit codes of `mochi-lock`, as in its `lock::exit`.
mod code {
    pub const UNLOCKED: i32 = 0;
    pub const REFUSED: i32 = 2;
    pub const UNSAFE: i32 = 3;
}

/// When to start `mochi-lock` again: never after unlocking or a refusal,
/// at once after the first crash, then waiting longer, up to 5 seconds, so
/// one that keeps crashing doesn't spin. While the screen is locked it
/// never gives up: a locked screen needs a lock screen.
#[derive(Debug, Default)]
struct Restarts {
    crashes: u32,
    last: Option<Instant>,
}

impl Restarts {
    fn after(&mut self, code: Option<i32>, now: Instant) -> Next {
        match code {
            Some(code::UNLOCKED | code::REFUSED | code::UNSAFE) => {
                self.crashes = 0;
                Next::Done
            }
            _ => {
                // A crash long after the last one starts the count again.
                if self
                    .last
                    .is_some_and(|last| now.duration_since(last) > Duration::from_secs(30))
                {
                    self.crashes = 0;
                }
                self.last = Some(now);
                self.crashes += 1;
                let wait = Duration::from_millis(250 * (1u64 << (self.crashes - 1).min(5)));
                Next::Respawn(wait.min(Duration::from_secs(5)))
            }
        }
    }
}

struct Locker {
    binary: PathBuf,
    config: PathBuf,
    marker: PathBuf,
    /// The `mochi-lock` this module started, by process id.
    child: Option<u32>,
    /// One found by its marker, which this module didn't start.
    adopted: Option<u32>,
    /// The compositor confirmed the lock.
    secured: bool,
    restarts: Restarts,
    respawn_at: Option<tokio::time::Instant>,
    /// Why the last lock didn't happen, for the status and the notice.
    problem: Option<String>,
    notice: Option<ActivityId>,
    session: Option<logind::SessionProxy<'static>>,
    published: Value,
    /// The renderer's shell, when it could be written.
    shell: Option<PathBuf>,
    /// The wallpaper, blurred, once it is.
    background: Option<PathBuf>,
    /// The last state of each module the lock screen shows, for a lock
    /// that starts later.
    states: std::collections::BTreeMap<String, Value>,
    /// Lines to the running lock's stdin.
    input: Option<mpsc::UnboundedSender<String>>,
    /// The daemon's QML tree, which the renderer's views are copied from.
    daemon_shell: Option<PathBuf>,
    /// The modules whose state is watched for the lock screen's widgets.
    watched: std::collections::BTreeSet<String>,
    /// Take a screenshot of each monitor before locking, for the background.
    screenshot: bool,
    /// Those screenshots, blurred, by monitor, until unlocked.
    shots: std::collections::BTreeMap<String, PathBuf>,
    /// Where they go: the module's data directory.
    shots_dir: PathBuf,
}

impl Locker {
    fn new(dir: &Path, settings: &Settings, ctx: &ModuleCtx) -> Self {
        let shell = ctx.data_dir().join("lock-shell");
        let shell = match screen::write_shell(&shell, ctx.shell_dir(), &[]) {
            Ok(()) => Some(shell),
            Err(error) => {
                tracing::warn!(%error, "can't write the lock screen's views; it shows its own face");
                None
            }
        };
        let locker = Self {
            binary: binary(),
            marker: dir.join("locked"),
            child: None,
            adopted: None,
            secured: false,
            restarts: Restarts::default(),
            respawn_at: None,
            problem: None,
            notice: None,
            session: None,
            published: Value::Null,
            shell,
            background: None,
            states: std::collections::BTreeMap::new(),
            input: None,
            daemon_shell: ctx.shell_dir().map(Path::to_owned),
            watched: std::collections::BTreeSet::new(),
            screenshot: settings.background == Background::Screenshot,
            shots: std::collections::BTreeMap::new(),
            shots_dir: ctx.data_dir().to_owned(),
            config: dir.join("lock.json"),
        };
        locker.write_config(settings, ctx);
        locker
    }

    fn write_config(&self, settings: &Settings, ctx: &ModuleCtx) {
        let renderer = match (screen::quickshell(), &self.shell) {
            (Some(quickshell), Some(shell)) => {
                Some(json!({ "quickshell": quickshell, "shell": shell }))
            }
            _ => None,
        };
        if let Err(error) = write_config(
            &self.config,
            settings,
            ctx,
            renderer,
            self.background.as_deref(),
        ) {
            tracing::warn!(%error, "can't write the lock's settings; it locks with its defaults");
        }
    }

    /// A module's state, for the lock screen: kept, and passed on to a
    /// running lock.
    fn relay(&mut self, ctx: &ModuleCtx, module: &str, state: Value) {
        // The widgets module's state, as the lock screen draws it: its
        // layout as `widgets`, never being arranged. The modules its widgets
        // are from are watched from then on, for their state.
        let state = if module == "widgets" {
            for used in layout_modules(&state) {
                if self.watched.insert(used.clone()) {
                    ctx.watch_state(&used);
                }
            }
            lock_widgets(&state)
        } else {
            state
        };
        let line = json!({ "type": "state", "module": module, "state": state }).to_string();
        self.states.insert(module.to_owned(), state);
        if let Some(input) = &self.input {
            let _ = input.send(line);
        }
    }

    fn running(&self) -> bool {
        self.child.is_some() || self.adopted.is_some()
    }

    fn status(&self) -> &'static str {
        match (self.secured, self.running()) {
            (true, _) => "locked",
            (false, true) => "locking",
            (false, false) => "unlocked",
        }
    }

    /// Starts `mochi-lock`, unless one already runs. With the screenshot
    /// background, each monitor is taken first, before anything locks: a
    /// lock started again after a crash keeps the ones it had.
    fn lock(&mut self, events: &mpsc::UnboundedSender<ChildEvent>) -> Result<(), String> {
        if self.running() {
            return Ok(());
        }
        self.respawn_at = None;
        if self.screenshot {
            self.shots = screen::shoot(&self.shots_dir);
        }
        self.spawn(events)
    }

    fn spawn(&mut self, events: &mpsc::UnboundedSender<ChildEvent>) -> Result<(), String> {
        // The views again: a module turned on since has its widgets too.
        if let Some(shell) = &self.shell
            && let Err(error) = screen::write_shell(
                shell,
                self.daemon_shell.as_deref(),
                &self.watched.iter().cloned().collect::<Vec<_>>(),
            )
        {
            tracing::warn!(%error, "can't write the lock screen's views");
        }
        let mut command = tokio::process::Command::new(&self.binary);
        command
            .arg("--config")
            .arg(&self.config)
            .arg("--marker")
            .arg(&self.marker)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            // It outlives this module: only the password ends it.
            .kill_on_drop(false);
        let mut child = command.spawn().map_err(|error| {
            let problem = format!("can't start {}: {error}", self.binary.display());
            self.problem = Some(problem.clone());
            problem
        })?;
        let Some(pid) = child.id() else {
            return Err("mochi-lock ended at once".into());
        };
        self.child = Some(pid);
        // The states it shows, now and as they change.
        let (input, mut lines_in) = mpsc::unbounded_channel::<String>();
        if let Some(mut stdin) = child.stdin.take() {
            tokio::spawn(async move {
                use tokio::io::AsyncWriteExt;
                while let Some(line) = lines_in.recv().await {
                    if stdin
                        .write_all(format!("{line}\n").as_bytes())
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }
        for (module, state) in &self.states {
            let _ = input
                .send(json!({ "type": "state", "module": module, "state": state }).to_string());
        }
        // The screenshots behind each monitor, with that background.
        if !self.shots.is_empty() {
            let _ =
                input.send(json!({ "type": "backgrounds", "backgrounds": self.shots }).to_string());
        }
        self.input = Some(input);
        let events = events.clone();
        let stdout = child.stdout.take();
        tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let mut lines = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = events.send(ChildEvent::Line(pid, line));
                }
            }
            let code = child.wait().await.ok().and_then(|status| status.code());
            let _ = events.send(ChildEvent::Exit(pid, code));
        });
        tracing::info!(pid, "started mochi-lock");
        Ok(())
    }

    /// At start: a marker means the screen is still locked. Its process
    /// still holds it, or else it died and the screen needs a new lock.
    async fn resume(&mut self, events: &mpsc::UnboundedSender<ChildEvent>) {
        let Some(pid) = marked(&self.marker) else {
            return;
        };
        if is_lock(pid) {
            tracing::info!(pid, "the screen is still locked; watching its lock");
            self.adopted = Some(pid);
            self.secured = true;
            self.hint(true).await;
        } else {
            tracing::warn!("the screen was locked and its lock is gone; locking again");
            let _ = self.spawn(events);
        }
    }

    async fn check_adopted(&mut self, events: &mpsc::UnboundedSender<ChildEvent>, ctx: &ModuleCtx) {
        let Some(pid) = self.adopted else {
            return;
        };
        if is_lock(pid) {
            return;
        }
        self.adopted = None;
        if marked(&self.marker).is_some() {
            // It died locked.
            self.secured = false;
            tracing::warn!(pid, "the lock screen died; starting it again");
            self.schedule(Next::Respawn(Duration::ZERO));
        } else {
            self.unlocked(ctx).await;
        }
        let _ = events;
    }

    async fn child_event(
        &mut self,
        event: ChildEvent,
        events: &mpsc::UnboundedSender<ChildEvent>,
        ctx: &ModuleCtx,
    ) {
        match event {
            ChildEvent::Line(pid, line) if self.child == Some(pid) => {
                let said: Value = serde_json::from_str(&line).unwrap_or_default();
                let reason = said["reason"].as_str().map(str::to_owned);
                match said["event"].as_str() {
                    Some("command") => {
                        let (module, action) = (
                            said["module"].as_str().unwrap_or_default(),
                            said["action"].as_str().unwrap_or_default(),
                        );
                        // Checked again here: mochi-lock lets only these through,
                        // and so does this.
                        if ALLOWED.contains(&(module, action)) {
                            let call = ctx.call(module, action, &[]);
                            tokio::spawn(async move {
                                let _ = call.await;
                            });
                        } else {
                            tracing::warn!(
                                module,
                                action,
                                "the lock screen asked for a command it may not run"
                            );
                        }
                    }
                    Some("locked") => {
                        self.secured = true;
                        self.problem = None;
                        self.hint(true).await;
                    }
                    Some("refused" | "broken" | "warning") => {
                        tracing::warn!(
                            reason = reason.as_deref().unwrap_or(""),
                            "mochi-lock: {}",
                            said["event"]
                        );
                        if said["event"] != "warning" {
                            self.problem = reason;
                        }
                    }
                    _ => {}
                }
            }
            ChildEvent::Exit(pid, code) if self.child == Some(pid) => {
                self.child = None;
                self.input = None;
                match code {
                    Some(code::UNLOCKED) => {
                        self.restarts.after(code, Instant::now());
                        self.unlocked(ctx).await;
                    }
                    Some(code::REFUSED | code::UNSAFE) => {
                        self.restarts.after(code, Instant::now());
                        self.secured = false;
                        let problem = self
                            .problem
                            .clone()
                            .unwrap_or_else(|| "the lock screen couldn't lock".into());
                        tracing::error!(%problem, "not locked");
                        self.notify(ctx, &problem);
                    }
                    _ => {
                        let was = self.secured;
                        self.secured = false;
                        let next = self.restarts.after(code, Instant::now());
                        tracing::warn!(
                            ?code,
                            locked = was,
                            "mochi-lock ended without unlocking; starting it again"
                        );
                        self.schedule(next);
                    }
                }
                let _ = events;
            }
            // From a lock that's no longer this module's.
            _ => {}
        }
    }

    fn schedule(&mut self, next: Next) {
        if let Next::Respawn(wait) = next {
            self.respawn_at = Some(tokio::time::Instant::now() + wait);
        }
    }

    async fn wait_to_respawn(&self) {
        match self.respawn_at {
            Some(at) => tokio::time::sleep_until(at).await,
            None => std::future::pending().await,
        }
    }

    fn respawn(&mut self, events: &mpsc::UnboundedSender<ChildEvent>) {
        self.respawn_at = None;
        if self.running() {
            return;
        }
        if let Err(error) = self.spawn(events) {
            tracing::error!(%error, "can't start the lock screen again");
            // Keep trying: the screen may be locked with nothing on it.
            self.schedule(Next::Respawn(Duration::from_secs(5)));
        }
    }

    async fn unlocked(&mut self, ctx: &ModuleCtx) {
        self.secured = false;
        // What was on screen goes as soon as it isn't needed.
        for shot in std::mem::take(&mut self.shots).into_values() {
            let _ = std::fs::remove_file(shot);
        }
        let _ = std::fs::remove_file(&self.marker);
        self.hint(false).await;
        if let Some(id) = self.notice.take() {
            ctx.withdraw(id);
        }
        tracing::info!("unlocked");
    }

    async fn hint(&self, locked: bool) {
        if let Some(session) = &self.session
            && let Err(error) = session.set_locked_hint(locked).await
        {
            tracing::debug!(%error, "can't set logind's LockedHint");
        }
    }

    /// Says on the island why the screen didn't lock.
    fn notify(&mut self, ctx: &ModuleCtx, problem: &str) {
        let spec = ActivitySpec::new("Notice")
            .key("notice")
            .priority(Priority::HIGH)
            .timeout(Duration::from_secs(12))
            .payload(json!({ "problem": problem }));
        self.notice = Some(ctx.present(spec));
    }

    fn publish(&mut self, ctx: &ModuleCtx) {
        let state = json!({
            "locked": self.secured,
            "status": self.status(),
            "problem": self.problem,
            // For the settings' menu of PAM services.
            "services": pam_services(Path::new("/etc/pam.d")),
            // The blurred wallpaper, for arranging the lock screen over it.
            "background": self.background,
        });
        if state != self.published {
            ctx.publish_state(state.clone());
            self.published = state;
        }
    }
}

/// PAM services a lock screen can check the password with, and what each
/// is, by the name of its file in `/etc/pam.d`.
const SERVICES: [(&str, &str); 7] = [
    ("mochi", "Mochi's own"),
    ("hyprlock", "hyprlock's"),
    ("swaylock", "swaylock's"),
    ("gtklock", "gtklock's"),
    ("waylock", "waylock's"),
    (
        "system-local-login",
        "The system's, for logging in at the screen",
    ),
    ("login", "The system's, for logging in"),
];

/// The PAM services from [`SERVICES`] that `dir` has, for the settings'
/// menu.
fn pam_services(dir: &Path) -> Vec<Value> {
    SERVICES
        .iter()
        .filter(|(name, _)| dir.join(name).is_file())
        .map(|(name, what)| json!({ "value": name, "label": name, "detail": what }))
        .collect()
}

/// `mochi-lock` beside mochid, as the packages install it and as `cargo
/// build` leaves it, or else from PATH.
fn binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("mochi-lock")))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("mochi-lock"))
}

/// The process id a marker holds.
fn marked(marker: &Path) -> Option<u32> {
    std::fs::read_to_string(marker).ok()?.trim().parse().ok()
}

/// Whether a process is alive and is `mochi-lock`, not something that took
/// its id since.
fn is_lock(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .is_ok_and(|name| name.trim() == "mochi-lock")
}

/// What `mochi-lock` reads: the PAM service, the font, the theme's colors
/// for its own face, and the renderer with the theme and the background.
fn write_config(
    path: &Path,
    settings: &Settings,
    ctx: &ModuleCtx,
    renderer: Option<Value>,
    background: Option<&Path>,
) -> std::io::Result<()> {
    let theme = ctx
        .config_dir()
        .map(|dir| dir.join("theme.toml"))
        .and_then(|file| mochi_core::config::load_theme(&file).ok())
        .unwrap_or_default();
    let theme_json = serde_json::to_value(&theme).unwrap_or(Value::Null);
    let colors = &theme.colors;
    let font = mochi_core::assets::find_fonts()
        .into_iter()
        .find(|path| path.ends_with("InterVariable.ttf"));
    let config = json!({
        "pam_service": settings.pam_service,
        "font": font,
        "colors": {
            "background": colors.background.as_str(),
            "raised": colors.raised.as_str(),
            "foreground": colors.foreground.as_str(),
            "muted": colors.muted.as_str(),
            "accent": colors.accent.as_str(),
            "danger": colors.danger.as_str(),
        },
        "renderer": renderer,
        "theme": theme_json,
        "background": background,
    });
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, config.to_string())
}

/// The modules the lock screen's widgets are from, but the lock module:
/// its state is mochi-lock's to give.
fn layout_modules(state: &Value) -> Vec<String> {
    let mut modules: Vec<String> = state["lockWidgets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|widget| widget["module"].as_str())
        .filter(|module| *module != "lock" && *module != "widgets")
        .map(str::to_owned)
        .collect();
    modules.sort();
    modules.dedup();
    modules
}

/// The widgets module's state as the lock screen's widgets view takes it:
/// the lock screen's layout in place of the desktop's, never arranged, and
/// none of the editor's data.
fn lock_widgets(state: &Value) -> Value {
    json!({
        "grid": state["grid"],
        "editing": false,
        "drawer": false,
        "widgets": state["lockWidgets"],
        "zones": state["zones"],
        "unknownZones": state["unknownZones"],
        "catalog": [],
        "layouts": [],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_again_after_a_crash_and_never_after_unlocking() {
        let mut restarts = Restarts::default();
        let now = Instant::now();
        assert_eq!(restarts.after(Some(code::UNLOCKED), now), Next::Done);
        assert_eq!(restarts.after(Some(code::REFUSED), now), Next::Done);
        assert_eq!(restarts.after(Some(code::UNSAFE), now), Next::Done);
        // Killed, or crashed: again, waiting longer each time, up to 5 s.
        assert_eq!(
            restarts.after(None, now),
            Next::Respawn(Duration::from_millis(250))
        );
        assert_eq!(
            restarts.after(Some(1), now),
            Next::Respawn(Duration::from_millis(500))
        );
        for _ in 0..10 {
            restarts.after(None, now);
        }
        assert_eq!(
            restarts.after(None, now),
            Next::Respawn(Duration::from_secs(5))
        );
        // A crash much later starts over.
        assert_eq!(
            restarts.after(None, now + Duration::from_secs(60)),
            Next::Respawn(Duration::from_millis(250))
        );
    }

    #[test]
    fn a_marker_holds_a_process_id_and_only_mochi_lock_counts() {
        let dir = std::env::temp_dir().join(format!("mochi-lock-marker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("locked");
        assert_eq!(marked(&marker), None);
        std::fs::write(&marker, format!("{}\n", std::process::id())).unwrap();
        assert_eq!(marked(&marker), Some(std::process::id()));
        // This test isn't mochi-lock, so its process doesn't count.
        assert!(!is_lock(std::process::id()));
        assert!(!is_lock(u32::MAX));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_lock_screen_gets_its_own_layout_never_arranged() {
        let state = json!({
            "grid": 16,
            "editing": true,
            "widgets": [{ "id": "w1" }],
            "lockWidgets": [{ "id": "lock-field" }],
            "catalog": [{ "widget": "note" }],
            "layouts": [{ "name": "Work" }],
        });
        let relayed = lock_widgets(&state);
        assert_eq!(relayed["widgets"], json!([{ "id": "lock-field" }]));
        assert_eq!(relayed["editing"], false);
        assert_eq!(relayed["catalog"], json!([]));
        assert_eq!(relayed["layouts"], json!([]));
    }

    #[test]
    fn lists_the_pam_services_there_are() {
        let dir = std::env::temp_dir().join(format!("mochi-pam-list-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hyprlock"), "").unwrap();
        std::fs::write(dir.join("sudo"), "").unwrap();
        let listed = pam_services(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["value"], "hyprlock");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_background_picks_what_to_blur() {
        let mut settings = Settings::default();
        assert_eq!(settings.backdrop().as_deref(), Some("auto"));
        settings.background = Background::Image;
        assert_eq!(settings.backdrop(), None, "no image named yet");
        settings.image = "~/Pictures/a.png".into();
        assert_eq!(settings.backdrop().as_deref(), Some("~/Pictures/a.png"));
        settings.background = Background::None;
        assert_eq!(settings.backdrop(), None);
    }

    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<Settings>("lock", include_str!("../settings.toml"));
    }
}

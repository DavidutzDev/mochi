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

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Assets, BoxFuture, Module, ModuleCtx, ModuleError,
    ModuleEvent, Priority,
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
    pam_service: String,
    /// Lock before the computer sleeps.
    before_sleep: bool,
    /// Lock when logind asks.
    logind: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pam_service: "mochi".into(),
            before_sleep: true,
            logind: true,
        }
    }
}

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

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("lock", "Lock the screen"),
            ActionSpec::new("status", "Say whether the screen is locked"),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let dir = ctx.session_dir().to_owned();
            let mut locker = Locker::new(&dir, &settings, &ctx);
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
}

impl Locker {
    fn new(dir: &Path, settings: &Settings, ctx: &ModuleCtx) -> Self {
        let config = dir.join("lock.json");
        if let Err(error) = write_config(&config, settings, ctx) {
            tracing::warn!(%error, "can't write the lock's settings; it locks with its defaults");
        }
        Self {
            binary: binary(),
            config,
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

    /// Starts `mochi-lock`, unless one already runs.
    fn lock(&mut self, events: &mpsc::UnboundedSender<ChildEvent>) -> Result<(), String> {
        if self.running() {
            return Ok(());
        }
        self.respawn_at = None;
        self.spawn(events)
    }

    fn spawn(&mut self, events: &mpsc::UnboundedSender<ChildEvent>) -> Result<(), String> {
        let mut command = tokio::process::Command::new(&self.binary);
        command
            .arg("--config")
            .arg(&self.config)
            .arg("--marker")
            .arg(&self.marker)
            .stdin(std::process::Stdio::null())
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
        });
        if state != self.published {
            ctx.publish_state(state.clone());
            self.published = state;
        }
    }
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

/// What `mochi-lock` reads: the PAM service, the font, and the theme's
/// colors for its face.
fn write_config(path: &Path, settings: &Settings, ctx: &ModuleCtx) -> std::io::Result<()> {
    let theme = ctx
        .config_dir()
        .map(|dir| dir.join("theme.toml"))
        .and_then(|file| mochi_core::config::load_theme(&file).ok())
        .unwrap_or_default();
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
    });
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, config.to_string())
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
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<Settings>("lock", include_str!("../settings.toml"));
    }
}

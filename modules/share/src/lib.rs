//! Share: the screen-share picker, and a bubble while something shares the
//! screen.
//!
//! xdg-desktop-portal-hyprland asks a program which screen, window or region
//! to share, and waits for it to print the answer. `mochi share-pick` is that
//! program: it runs this module's `pick`, which holds the command while the
//! island shows the screens and the windows with live thumbnails, plus a
//! region drawn over the screen. The answer goes back as the command's
//! output, in the portal's format: `[SELECTION]<flags>/<choice>`, where the
//! choice is `screen:DP-3`, `window:<handle>` or `region:DP-3@x,y,w,h`, and
//! the flag `r` lets the app keep the choice for next time. An empty answer
//! cancels.
//!
//! With `switchable` on, the app gets a monitor of Mochi's own instead,
//! with a live copy of the choice on it, and clicking the bubble picks
//! another source for the copy: see [`switch`].
//!
//! The bubble follows the compositor's screencast state, which Hyprland
//! reports on its event socket.
//!
//! A restart of mochid doesn't end a share: see [`saved`].

mod saved;
mod switch;
mod tour;
mod windows;

use std::time::{Duration, Instant};

use include_dir::{Dir, include_dir};
use mochi_core::quality::{self, Resolution};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Args, Assets, BoxFuture, BubbleId,
    BubbleSpec, ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent,
    Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::saved::Saved;
use crate::switch::{OUTPUT, Session, Source};
use crate::windows::Window;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Share;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    switchable: bool,
    framerate: u32,
    resolution: Resolution,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            switchable: true,
            framerate: 60,
            resolution: Resolution::Native,
        }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if !(1..=MAX_FRAMERATE).contains(&settings.framerate) {
            return Err(format!(
                "framerate is {}; it goes from 1 to {MAX_FRAMERATE}",
                settings.framerate
            ));
        }
        Ok(settings)
    }
}

/// The highest refresh rate the switchable monitor may have.
const MAX_FRAMERATE: u32 = 240;

impl Module for Share {
    fn id(&self) -> &'static str {
        "share"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        tour::steps()
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        Settings::load(table).map(drop)
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new(
                "pick",
                "Ask what to share and print the portal's answer; `mochi share-pick` runs this",
            )
            .arg(ArgSpec::bool(
                "remember",
                "Whether the app may keep the choice",
            ))
            .arg(
                ArgSpec::string("windows", "The portal's XDPH_WINDOW_SHARING_LIST")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new("screen", "Share a screen; the picker sends this")
                .arg(ArgSpec::string("output", "The monitor")),
            ActionSpec::new("window", "Share a window; the picker sends this").arg(
                ArgSpec::string(
                    "handle",
                    "The portal's handle for it, or Hyprland's address when switching",
                ),
            ),
            ActionSpec::new("region", "Draw the area to share instead"),
            ActionSpec::new("back", "Go back from drawing an area to the list"),
            ActionSpec::new("area", "Share this area; the overlay sends this")
                .arg(ArgSpec::string("output", "The monitor it's on"))
                .arg(ArgSpec::int("x", "Left edge, on the monitor"))
                .arg(ArgSpec::int("y", "Top edge, on the monitor"))
                .arg(ArgSpec::int("width", "In logical pixels"))
                .arg(ArgSpec::int("height", "In logical pixels")),
            ActionSpec::new("remember", "Turn keeping the choice on or off"),
            ActionSpec::new(
                "switchable",
                "Turn sharing a switchable copy on or off, for this share",
            ),
            ActionSpec::new(
                "framerate",
                "Step the switchable share's frame rate to the next preset",
            ),
            ActionSpec::new(
                "resolution",
                "Step the switchable share's resolution to the next preset",
            ),
            ActionSpec::new(
                "switch",
                "Pick another source for the switchable share; the bubble does this",
            ),
            ActionSpec::new("cancel", "Share nothing"),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let mut compositor = ctx.compositor().subscribe();
            // Until the compositor connection goes away.
            let mut following = true;
            let mut state = State {
                settings: ctx.settings()?,
                ..State::default()
            };
            state.resume(&ctx).await;
            state.capturing(&ctx, &compositor.borrow());
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => {
                            state.finish(&ctx, String::new());
                            if state.session.as_ref().is_some_and(Session::is_captured) {
                                // The app still captures the monitor; the
                                // next start carries on with the share.
                                tracing::info!("leaving the switchable monitor to the next start");
                                state.save(&ctx);
                            } else {
                                state.stop(&ctx).await;
                            }
                            return Ok(());
                        }
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, .. }) => state.ended(&ctx, activity),
                        Some(ModuleEvent::BubbleClicked(_)) => state.switch(&ctx),
                        Some(_) => {}
                    },
                    changed = compositor.changed(), if following => {
                        if changed.is_err() {
                            following = false;
                            continue;
                        }
                        let snapshot = compositor.borrow_and_update().clone();
                        state.capturing(&ctx, &snapshot);
                    }
                    () = sleep_until(state.bubble_due()) => state.update_bubble(&ctx),
                    () = sleep_until(state.session_due()) => {
                        if state.session.as_ref().is_some_and(|session| session.ended(Instant::now())) {
                            tracing::info!("the switchable share ended");
                            state.stop(&ctx).await;
                        }
                    }
                }
            }
        })
    }
}

/// How long a capture must last before the bubble believes it: a
/// screenshot's one-frame capture is far shorter, a screen share far longer.
const BELIEVE_AFTER: Duration = Duration::from_millis(1500);

/// Waits until `deadline`, or forever without one.
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
        None => std::future::pending().await,
    }
}

/// A `pick` waiting for the user.
#[derive(Debug)]
struct Pick {
    /// The portal's request; `None` when switching a running share.
    command: Option<ModuleCommand>,
    remember: bool,
    /// Share a switchable copy rather than the choice itself.
    switchable: bool,
    /// The switchable copy's quality.
    framerate: u32,
    resolution: Resolution,
    windows: Vec<Window>,
    /// Drawing an area over the screen, rather than choosing from the list.
    drawing: bool,
    /// The monitor with the keyboard.
    output: Option<String>,
    activity: ActivityId,
}

impl Pick {
    fn payload(&self) -> Value {
        json!({
            "remember": self.remember,
            "switchable": self.switchable,
            "framerate": self.framerate,
            "resolution": self.resolution.as_str(),
            "switching": self.command.is_none(),
            "windows": self.windows.iter().map(Window::to_json).collect::<Vec<_>>(),
            "output": self.output,
            "drawing": self.drawing,
        })
    }

    fn spec(&self) -> ActivitySpec {
        let spec = ActivitySpec::new(if self.drawing { "Drawing" } else { "Picker" })
            .key("picker")
            .priority(Priority::URGENT)
            .uninterruptible()
            .payload(self.payload());
        if self.drawing {
            spec.overlay("Region")
        } else {
            spec.modal()
        }
    }

    /// The portal's answer for `choice`.
    fn answer(&self, choice: &str) -> String {
        let flags = if self.remember { "r" } else { "" };
        format!("[SELECTION]{flags}/{choice}")
    }
}

#[derive(Debug, Default)]
struct State {
    settings: Settings,
    pick: Option<Pick>,
    bubble: Option<BubbleId>,
    /// Since when the compositor reports a capture, while it does.
    capturing_since: Option<Instant>,
    /// The switchable share running now.
    session: Option<Session>,
    /// What the compositor said was captured, when last saved.
    saved_captures: Vec<String>,
}

impl State {
    /// Picks up where the last run left off: gives the compositor back the
    /// captures that were running, and carries on with the switchable share
    /// while its monitor is still there. A monitor without a share to go
    /// with it, left by a crash, shares nothing now and goes.
    async fn resume(&mut self, ctx: &ModuleCtx) {
        let saved = Saved::load(ctx.session_dir()).unwrap_or_default();
        ctx.compositor().assume_captures(saved.captured.clone());
        let exists = outputs(ctx).iter().any(|(name, ..)| name == OUTPUT);
        let session = saved
            .session
            .as_ref()
            .and_then(|session| session.resume(Instant::now()));
        match session {
            Some(session) if exists => {
                tracing::info!(source = ?session.source, "carrying on with the switchable share");
                ctx.publish_state(session.payload());
                self.session = Some(session);
            }
            _ => self.stop(ctx).await,
        }
    }

    /// Writes down what a restart needs to carry on.
    fn save(&mut self, ctx: &ModuleCtx) {
        let captured = ctx.compositor().state().captured;
        Saved::new(captured.clone(), self.session.as_ref()).save(ctx.session_dir());
        self.saved_captures = captured;
    }

    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let result = match command.action.as_str() {
            "pick" => {
                self.open(ctx, command);
                return;
            }
            "screen" => match args.str("output") {
                Some(output) => {
                    // The portal drops the last character of a screen's name.
                    let portal = format!("screen:{output}\n");
                    self.choose(ctx, Some(Source::Screen(output.to_owned())), portal)
                        .await
                }
                None => Err("bad choice".into()),
            },
            "window" => match args.str("handle") {
                Some(handle) => {
                    let source = self.window(handle);
                    self.choose(ctx, source, format!("window:{handle}")).await
                }
                None => Err("bad choice".into()),
            },
            "area" => match (area(args), area_source(args)) {
                (Some(portal), source) => self.choose(ctx, source, portal).await,
                (None, _) => Err("bad choice".into()),
            },
            "region" => self.draw(ctx, true),
            "back" => self.draw(ctx, false),
            "remember" => match &mut self.pick {
                Some(pick) => {
                    pick.remember = !pick.remember;
                    ctx.update(pick.activity, pick.payload());
                    Ok(())
                }
                None => Err("nothing is being shared".into()),
            },
            "switchable" => self.change(ctx, |pick| pick.switchable = !pick.switchable),
            // The copy's monitor is made once, so only a new share picks it.
            "framerate" => match self.change(ctx, |pick| {
                pick.framerate = quality::next_framerate(pick.framerate);
            }) {
                Ok(()) => self.resize(ctx).await,
                Err(error) => Err(error),
            },
            "resolution" => match self.change(ctx, |pick| pick.resolution = pick.resolution.next())
            {
                Ok(()) => self.resize(ctx).await,
                Err(error) => Err(error),
            },
            "switch" => {
                if self.session.is_some() {
                    self.switch(ctx);
                    Ok(())
                } else {
                    Err("nothing is shared switchably; turn on Switchable when you share".into())
                }
            }
            "cancel" => {
                self.finish(ctx, String::new());
                Ok(())
            }
            other => Err(format!("share has no action {other}")),
        };
        command.reply(result);
    }

    /// Changes the open picker's choices.
    fn change(&mut self, ctx: &ModuleCtx, change: impl FnOnce(&mut Pick)) -> Result<(), String> {
        let pick = self.pick.as_mut().ok_or("nothing is being shared")?;
        change(pick);
        ctx.update(pick.activity, pick.payload());
        Ok(())
    }

    fn open(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        // A newer request wins; the older one shares nothing.
        self.finish(ctx, String::new());
        let remember = command.args.bool("remember").unwrap_or(false);
        let windows = windows::parse(command.args.str("windows").unwrap_or_default());
        let mut pick = Pick {
            command: Some(command),
            remember,
            // Only Hyprland makes monitors on request.
            switchable: self.settings.switchable && ctx.compositor().knows_windows(),
            framerate: self.settings.framerate,
            resolution: self.settings.resolution,
            windows,
            drawing: false,
            output: ctx.compositor().state().focused_output,
            activity: ActivityId(0),
        };
        pick.activity = ctx.present(pick.spec());
        self.pick = Some(pick);
        // Its thumbnails capture the screen too.
        self.update_bubble(ctx);
    }

    /// Opens the picker again for the switchable share, to change its
    /// source. A share that isn't switchable can't change.
    fn switch(&mut self, ctx: &ModuleCtx) {
        let Some(session) = &self.session else {
            return;
        };
        if self.pick.is_some() {
            return;
        }
        let mut pick = Pick {
            command: None,
            remember: false,
            switchable: true,
            framerate: session.framerate,
            resolution: session.resolution,
            windows: Vec::new(),
            drawing: false,
            output: ctx.compositor().state().focused_output,
            activity: ActivityId(0),
        };
        pick.activity = ctx.present(pick.spec());
        self.pick = Some(pick);
    }

    /// While switching a running share, gives its monitor the quality the
    /// picker now shows. The app sees the stream change size or pace, as
    /// when a shared window is resized.
    async fn resize(&mut self, ctx: &ModuleCtx) -> Result<(), String> {
        let Some(pick) = self.pick.as_ref().filter(|pick| pick.command.is_none()) else {
            return Ok(());
        };
        let Some(session) = self.session.as_mut() else {
            return Ok(());
        };
        let (framerate, resolution) = (pick.framerate, pick.resolution);
        let (width, height) = resolution.fit_within(session.size);
        ctx.compositor()
            .resize_virtual_output(OUTPUT, width, height, framerate)
            .await
            .map_err(|error| error.to_string())?;
        tracing::info!(
            width,
            height,
            framerate,
            "changed the switchable share's quality"
        );
        session.framerate = framerate;
        session.resolution = resolution;
        self.save(ctx);
        Ok(())
    }

    /// The source for a window the picker sent: by the portal's handle for
    /// a new share, by Hyprland's address when switching.
    fn window(&self, handle: &str) -> Option<Source> {
        let pick = self.pick.as_ref()?;
        if pick.command.is_none() {
            return Some(Source::Window {
                address: handle.to_owned(),
                title: String::new(),
            });
        }
        let window = pick.windows.iter().find(|window| window.handle == handle)?;
        (!window.address.is_empty()).then(|| Source::Window {
            address: window.address.clone(),
            title: window.title.clone(),
        })
    }

    /// Shares the choice: switches the running share to it, or answers the
    /// portal with the switchable monitor, or with the choice itself when
    /// that's off or fails. `source` is `None` for what can't be copied.
    async fn choose(
        &mut self,
        ctx: &ModuleCtx,
        source: Option<Source>,
        portal: String,
    ) -> Result<(), String> {
        let pick = self.pick.as_ref().ok_or("nothing is being shared")?;
        if pick.command.is_none() {
            let source = source.ok_or("Mochi can't copy that window")?;
            tracing::info!(?source, "switching the share");
            if let Some(session) = &mut self.session {
                session.source = source;
                ctx.publish_state(session.payload());
            }
            self.save(ctx);
            self.finish(ctx, String::new());
            return Ok(());
        }

        if pick.switchable
            && let Some(source) = source
        {
            let quality = (pick.framerate, pick.resolution);
            match self.start(ctx, source, quality).await {
                Ok(()) => {
                    tracing::info!("sharing the switchable monitor");
                    self.save(ctx);
                    self.finish(ctx, format!("[SELECTION]/screen:{OUTPUT}\n"));
                    return Ok(());
                }
                Err(error) => {
                    tracing::warn!(%error, "can't share switchably; sharing the choice itself");
                }
            }
        }
        let pick = self.pick.as_ref().ok_or("nothing is being shared")?;
        let answer = pick.answer(&portal);
        tracing::info!(choice = portal.trim_end(), "sharing");
        self.finish(ctx, answer);
        Ok(())
    }

    /// Makes the switchable monitor with a copy of `source` on it, and waits
    /// until it's there for the portal to find.
    async fn start(
        &mut self,
        ctx: &ModuleCtx,
        source: Source,
        (framerate, resolution): (u32, Resolution),
    ) -> Result<(), String> {
        let compositor = ctx.compositor();
        let outputs = outputs(ctx);
        let size = switch::size(&outputs, &source);
        let mut session = Session::new(source, Instant::now());
        session.framerate = framerate;
        session.resolution = resolution;
        session.size = size;
        ctx.publish_state(session.payload());
        self.session = Some(session);
        let (width, height) = resolution.fit_within(size);
        if outputs.iter().any(|(name, ..)| name == OUTPUT) {
            // Left from the last share, maybe in another shape.
            compositor
                .resize_virtual_output(OUTPUT, width, height, framerate)
                .await
                .map_err(|error| error.to_string())?;
        } else {
            tracing::info!(width, height, framerate, "making the switchable monitor");
            compositor
                .create_virtual_output(OUTPUT, width, height, framerate)
                .await
                .map_err(|error| error.to_string())?;
        }
        let mut changes = compositor.subscribe();
        let appeared = tokio::time::timeout(APPEAR, async {
            loop {
                if changes
                    .borrow_and_update()
                    .outputs
                    .iter()
                    .any(|output| output.name == OUTPUT)
                {
                    return;
                }
                if changes.changed().await.is_err() {
                    std::future::pending::<()>().await;
                }
            }
        })
        .await;
        if appeared.is_err() {
            self.stop(ctx).await;
            return Err(format!("{OUTPUT} never appeared"));
        }
        // A moment for the copy to draw its first frame.
        tokio::time::sleep(FIRST_FRAME).await;
        Ok(())
    }

    /// Ends the switchable share and removes its monitor.
    async fn stop(&mut self, ctx: &ModuleCtx) {
        self.session = None;
        ctx.publish_state(Value::Null);
        self.save(ctx);
        let exists = ctx
            .compositor()
            .state()
            .outputs
            .iter()
            .any(|output| output.name == OUTPUT);
        if exists && let Err(error) = ctx.compositor().remove_virtual_output(OUTPUT).await {
            tracing::warn!(%error, "can't remove the switchable monitor");
        }
    }

    /// When the switchable share may be over.
    fn session_due(&self) -> Option<Instant> {
        self.session.as_ref().and_then(Session::deadline)
    }

    fn draw(&mut self, ctx: &ModuleCtx, drawing: bool) -> Result<(), String> {
        let pick = self.pick.as_mut().ok_or("nothing is being shared")?;
        pick.drawing = drawing;
        pick.activity = ctx.present(pick.spec());
        Ok(())
    }

    /// Answers the waiting `pick`, if any, and closes the picker.
    fn finish(&mut self, ctx: &ModuleCtx, answer: String) {
        if let Some(pick) = self.pick.take() {
            ctx.withdraw(pick.activity);
            if let Some(command) = pick.command {
                command.answer(Ok(answer));
            }
            // The thumbnails' captures are ending; time a real share afresh, or
            // the bubble would flash before they do.
            if self.capturing_since.is_some() {
                self.capturing_since = Some(Instant::now());
            }
            self.update_bubble(ctx);
        }
    }

    fn ended(&mut self, ctx: &ModuleCtx, activity: ActivityId) {
        // Dismissed, by a click outside or Escape: share nothing.
        if self
            .pick
            .as_ref()
            .is_some_and(|pick| pick.activity == activity)
        {
            self.finish(ctx, String::new());
        }
    }

    /// Follows the compositor's capture state.
    fn capturing(&mut self, ctx: &ModuleCtx, state: &mochi_core::compositor::State) {
        if let Some(session) = &mut self.session {
            let shared = state.captured.iter().any(|target| target == OUTPUT);
            session.captured(shared, Instant::now());
        }
        if state.captured != self.saved_captures {
            self.save(ctx);
        }
        let active = state.screencast;
        self.capturing_since = match (active, self.capturing_since) {
            (true, None) => Some(Instant::now()),
            (true, since) => since,
            (false, _) => None,
        };
        self.update_bubble(ctx);
    }

    /// When the bubble should appear, if it's waiting to.
    fn bubble_due(&self) -> Option<Instant> {
        let since = self.capturing_since?;
        (self.bubble.is_none() && self.pick.is_none()).then_some(since + BELIEVE_AFTER)
    }

    /// Shows the bubble while the screen has been captured for a while, and
    /// not by the picker's own thumbnails.
    fn update_bubble(&mut self, ctx: &ModuleCtx) {
        let shared = self.pick.is_none()
            && self
                .capturing_since
                .is_some_and(|since| since.elapsed() >= BELIEVE_AFTER);
        match (shared, self.bubble) {
            (true, None) => {
                tracing::info!("the screen is being shared");
                self.bubble = Some(
                    ctx.show_bubble(
                        BubbleSpec::new("Sharing")
                            .key("sharing")
                            .area(Area::CenterRight)
                            .order(-9)
                            .payload(json!({ "switchable": self.session.is_some() })),
                    ),
                );
            }
            (false, Some(bubble)) => {
                tracing::info!("the screen is no longer shared");
                ctx.hide_bubble(bubble);
                self.bubble = None;
            }
            _ => {}
        }
    }
}

/// How long the switchable monitor has to appear.
const APPEAR: Duration = Duration::from_secs(3);
/// How long the copy gets to draw before the app sees the monitor.
const FIRST_FRAME: Duration = Duration::from_millis(300);

/// Every monitor's name and size in pixels.
fn outputs(ctx: &ModuleCtx) -> Vec<(String, u32, u32)> {
    ctx.compositor()
        .state()
        .outputs
        .iter()
        .map(|output| (output.name.clone(), output.width, output.height))
        .collect()
}

/// The copy of an area the picker sent.
fn area_source(args: &Args) -> Option<Source> {
    Some(Source::Area {
        output: args.str("output")?.to_owned(),
        x: args.int("x")?,
        y: args.int("y")?,
        width: args.int("width")?.max(1),
        height: args.int("height")?.max(1),
    })
}

/// `region:<output>@x,y,w,h`, on the monitor, in logical pixels.
fn area(args: &Args) -> Option<String> {
    let (width, height) = (args.int("width")?, args.int("height")?);
    if width <= 0 || height <= 0 {
        return None;
    }
    let (output, x, y) = (args.str("output")?, args.int("x")?, args.int("y")?);
    Some(format!("region:{output}@{x},{y},{width},{height}"))
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "share",
            include_str!("../settings.toml"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &str) -> Args {
        let spec = ActionSpec::new("area", "")
            .arg(ArgSpec::string("output", ""))
            .arg(ArgSpec::int("x", ""))
            .arg(ArgSpec::int("y", ""))
            .arg(ArgSpec::int("width", ""))
            .arg(ArgSpec::int("height", ""));
        let words: Vec<String> = words.split(' ').map(str::to_owned).collect();
        mochi_core::actions::parse(&spec, &words).unwrap()
    }

    #[test]
    fn writes_regions_the_portal_reads() {
        assert_eq!(
            area(&args("DP-3 100 50 640 360")).as_deref(),
            Some("region:DP-3@100,50,640,360")
        );
        assert_eq!(area(&args("DP-3 100 50 0 360")), None);
    }

    #[test]
    fn answers_with_the_remember_flag() {
        let (command, _) = ModuleCommand::new("pick".into(), Args::default());
        let mut pick = Pick {
            command: Some(command),
            remember: false,
            switchable: false,
            framerate: 60,
            resolution: Resolution::Native,
            windows: Vec::new(),
            drawing: false,
            output: None,
            activity: ActivityId(1),
        };
        assert_eq!(pick.answer("window:12"), "[SELECTION]/window:12");
        pick.remember = true;
        assert_eq!(pick.answer("screen:DP-3\n"), "[SELECTION]r/screen:DP-3\n");
    }
}

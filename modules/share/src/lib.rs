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
//! The bubble follows the compositor's screencast state, which Hyprland
//! reports on its event socket.

mod windows;

use std::time::{Duration, Instant};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Args, Assets, BoxFuture, BubbleId,
    BubbleSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde_json::{Value, json};

use crate::windows::Window;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Share;

impl Module for Share {
    fn id(&self) -> &'static str {
        "share"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
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
            ActionSpec::new("window", "Share a window; the picker sends this")
                .arg(ArgSpec::string("handle", "The portal's handle for it")),
            ActionSpec::new("region", "Draw the area to share instead"),
            ActionSpec::new("back", "Go back from drawing an area to the list"),
            ActionSpec::new("area", "Share this area; the overlay sends this")
                .arg(ArgSpec::string("output", "The monitor it's on"))
                .arg(ArgSpec::int("x", "Left edge, on the monitor"))
                .arg(ArgSpec::int("y", "Top edge, on the monitor"))
                .arg(ArgSpec::int("width", "In logical pixels"))
                .arg(ArgSpec::int("height", "In logical pixels")),
            ActionSpec::new("remember", "Turn keeping the choice on or off"),
            ActionSpec::new("cancel", "Share nothing"),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let mut compositor = ctx.compositor().subscribe();
            // Until the compositor connection goes away.
            let mut following = true;
            let mut state = State::default();
            state.capturing(&ctx, compositor.borrow().screencast);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => {
                            state.finish(&ctx, String::new());
                            return Ok(());
                        }
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::Ended { activity, .. }) => state.ended(&ctx, activity),
                        Some(_) => {}
                    },
                    changed = compositor.changed(), if following => {
                        if changed.is_err() {
                            following = false;
                            continue;
                        }
                        let screencast = compositor.borrow_and_update().screencast;
                        state.capturing(&ctx, screencast);
                    }
                    () = sleep_until(state.bubble_due()) => state.update_bubble(&ctx),
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
    command: ModuleCommand,
    remember: bool,
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
    pick: Option<Pick>,
    bubble: Option<BubbleId>,
    /// Since when the compositor reports a capture, while it does.
    capturing_since: Option<Instant>,
}

impl State {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let result = match command.action.as_str() {
            "pick" => {
                self.open(ctx, command);
                return;
            }
            "screen" => self.choose(ctx, args, |args| {
                // The portal drops the last character of a screen's name.
                Some(format!("screen:{}\n", args.str("output")?))
            }),
            "window" => self.choose(ctx, args, |args| {
                Some(format!("window:{}", args.str("handle")?))
            }),
            "area" => self.choose(ctx, args, area),
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
            "cancel" => {
                self.finish(ctx, String::new());
                Ok(())
            }
            other => Err(format!("share has no action {other}")),
        };
        command.reply(result);
    }

    fn open(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        // A newer request wins; the older one shares nothing.
        self.finish(ctx, String::new());
        let remember = command.args.bool("remember").unwrap_or(false);
        let windows = windows::parse(command.args.str("windows").unwrap_or_default());
        let mut pick = Pick {
            command,
            remember,
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

    fn choose(
        &mut self,
        ctx: &ModuleCtx,
        args: &Args,
        choice: impl FnOnce(&Args) -> Option<String>,
    ) -> Result<(), String> {
        let pick = self.pick.as_ref().ok_or("nothing is being shared")?;
        let choice = choice(args).ok_or("bad choice")?;
        let answer = pick.answer(&choice);
        tracing::info!(choice = choice.trim_end(), "sharing");
        self.finish(ctx, answer);
        Ok(())
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
            pick.command.answer(Ok(answer));
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
    fn capturing(&mut self, ctx: &ModuleCtx, active: bool) {
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
                            .order(-9),
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
            command,
            remember: false,
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

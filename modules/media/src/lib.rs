//! Now playing, from any player that speaks MPRIS: Spotify, browsers, mpv
//! with mpv-mpris, VLC and most others.
//!
//! A new track takes the island: the expanded view with the cover, progress
//! and controls for a few seconds, then the compact view for a few more.
//! After that the island goes back to whatever it showed, and the music
//! stays as a bubble next to it. Clicking the bubble opens the player on the
//! island again. A pause dims the bubble, which leaves after a while.
//!
//! With several players, arrows next to the player's name switch between
//! them. The one picked stays shown until it stops, even when another starts
//! a new track.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.media]
//! expand_ms = 4000   # how long a new track shows the expanded view
//! island_ms = 3000   # then how long the compact view stays before the bubble
//! paused_ms = 3000   # how long a paused player's bubble stays
//! ignore = []        # players never shown, like ["firefox"]
//!
//! [bubbles.media]    # where the bubble goes; center-left by default
//! area = "left"
//! wide = true        # the title next to the cover, instead of the cover alone
//! ```

mod mpris;
mod notice;

use std::time::{Duration, Instant};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use zbus::Connection;

use crate::mpris::{Control, Status};
use crate::notice::{Notice, Tracker};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// One activity and one bubble, each replaced in place as playback changes.
const KEY: &str = "media";

#[derive(Debug, Default)]
pub struct Media;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    expand_ms: u64,
    island_ms: u64,
    paused_ms: u64,
    ignore: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            expand_ms: 4000,
            island_ms: 3000,
            paused_ms: 3000,
            ignore: Vec::new(),
        }
    }
}

impl Module for Media {
    fn id(&self) -> &'static str {
        "media"
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

    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("hub", "card", "now-playing", "Card", "Now playing")
                .icon("music")
                .order(10)
                .options(json!({ "span": 2 })),
            // The same card on the desktop; it steps aside when nothing plays.
            ContributionSpec::new("widgets", "widget", "now-playing", "Card", "Now playing")
                .icon("music")
                .options(json!({ "size": [24, 8], "min": [18, 7], "max": [40, 9] })),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("play-pause", "Toggle playback of the shown player"),
            ActionSpec::new("play", "Start playback of the shown player"),
            ActionSpec::new("pause", "Pause the shown player"),
            ActionSpec::new("next", "Skip to the next track"),
            ActionSpec::new("previous", "Go back to the previous track"),
            ActionSpec::new("seek", "Jump to a position in the current track").arg(ArgSpec::float(
                "position",
                "Seconds from the start of the track",
            )),
            ActionSpec::new("next-player", "Show the next player, until it stops"),
            ActionSpec::new(
                "previous-player",
                "Show the previous player, until it stops",
            ),
            ActionSpec::new("player", "Show a player, until it stops").arg(ArgSpec::string(
                "name",
                "The player's name, like spotify or firefox",
            )),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let connection = Connection::session().await?;

            let (sender, mut updates) = mpsc::unbounded_channel();
            tokio::spawn(mpris::watch(connection.clone(), sender));

            let mut tracker = Tracker::new(&settings.ignore);
            let mut screen = Screen::default();
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            let switch = match command.action.as_str() {
                                "next-player" => Some(tracker.step(1)),
                                "previous-player" => Some(tracker.step(-1)),
                                "player" => Some(tracker.pick(command.args.str("name").unwrap_or_default())),
                                _ => None,
                            };
                            match switch {
                                Some(Ok(notice)) => {
                                    command.reply(Ok(()));
                                    if let Some(notice) = notice {
                                        ctx.publish_state(tracker.payload());
                                        screen.apply(&ctx, &settings, notice, &tracker);
                                    }
                                }
                                Some(Err(error)) => command.reply(Err(error)),
                                None => control(&connection, &tracker, command),
                            }
                        }
                        Some(ModuleEvent::Ended { activity, .. }) => {
                            screen.ended(&ctx, &settings, activity, &tracker);
                        }
                        Some(ModuleEvent::BubbleClicked(_)) => {
                            if tracker.chosen().is_some() {
                                screen.open(&ctx, &settings, &tracker);
                            }
                        }
                        Some(_) => {}
                    },
                    Some(update) = updates.recv() => {
                        let notice = tracker.apply(update);
                        // For views outside the island, like the hub's card.
                        ctx.publish_state(tracker.payload());
                        let Some(notice) = notice else { continue };
                        tracing::debug!(?notice, "media");
                        screen.apply(&ctx, &settings, notice, &tracker);
                    }
                    () = sleep_until(screen.bubble_ends) => screen.hide_bubble(&ctx),
                }
            }
        })
    }
}

/// What the module has on screen: the player on the island after a track
/// change or a click, and the bubble the rest of the time, never both.
#[derive(Debug, Default)]
struct Screen {
    activity: Option<ActivityId>,
    bubble: Option<BubbleId>,
    /// When a paused player's bubble leaves.
    bubble_ends: Option<Instant>,
}

impl Screen {
    fn apply(&mut self, ctx: &ModuleCtx, settings: &Settings, notice: Notice, tracker: &Tracker) {
        if notice == Notice::Hide || tracker.chosen().is_none() {
            if let Some(id) = self.activity.take() {
                ctx.withdraw(id);
            }
            self.hide_bubble(ctx);
            return;
        };
        match (notice, self.activity, self.bubble) {
            (Notice::Track, ..) => self.open(ctx, settings, tracker),
            (_, Some(activity), _) => ctx.update(activity, tracker.payload()),
            (Notice::Refresh, None, Some(bubble)) => {
                ctx.update_bubble(bubble, tracker.payload());
            }
            (Notice::Refresh, None, None) => {}
            // Playing, paused or switched: the bubble shows it, and a paused
            // one starts its countdown.
            _ => self.show_bubble(ctx, settings, tracker),
        }
    }

    /// Puts the player on the island, opening on the expanded view.
    fn open(&mut self, ctx: &ModuleCtx, settings: &Settings, tracker: &Tracker) {
        self.hide_bubble(ctx);
        let spec = ActivitySpec::new("Compact")
            .expanded("Expanded")
            .key(KEY)
            .priority(Priority::LOW)
            .expand_for(Duration::from_millis(settings.expand_ms))
            .timeout(Duration::from_millis(settings.island_ms))
            .payload(tracker.payload());
        self.activity = Some(ctx.present(spec));
    }

    /// The island is done with the player: the bubble takes over.
    fn ended(
        &mut self,
        ctx: &ModuleCtx,
        settings: &Settings,
        activity: ActivityId,
        tracker: &Tracker,
    ) {
        if self.activity != Some(activity) {
            return;
        }
        self.activity = None;
        self.show_bubble(ctx, settings, tracker);
    }

    fn show_bubble(&mut self, ctx: &ModuleCtx, settings: &Settings, tracker: &Tracker) {
        let Some((_, player)) = tracker.chosen() else {
            return;
        };
        let spec = BubbleSpec::new("Bubble")
            .wide("BubbleWide")
            .key(KEY)
            .area(Area::CenterLeft)
            .payload(tracker.payload());
        self.bubble = Some(ctx.show_bubble(spec));
        self.bubble_ends = (player.status != Status::Playing)
            .then(|| Instant::now() + Duration::from_millis(settings.paused_ms));
    }

    fn hide_bubble(&mut self, ctx: &ModuleCtx) {
        if let Some(id) = self.bubble.take() {
            ctx.hide_bubble(id);
        }
        self.bubble_ends = None;
    }
}

/// Waits until `deadline`, or forever without one.
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
        None => std::future::pending().await,
    }
}

/// Runs a command on the shown player. The reply comes once the player
/// answered, without holding up the module.
fn control(connection: &Connection, tracker: &Tracker, command: ModuleCommand) {
    let Some((bus, player)) = tracker.chosen() else {
        command.reply(Err("no media player is playing anything".into()));
        return;
    };
    let action = command.action.clone();
    let control = match action.as_str() {
        "play-pause" => Control::PlayPause,
        "play" => Control::Play,
        "pause" => Control::Pause,
        "next" => Control::Next,
        "previous" => Control::Previous,
        "seek" => {
            let seconds = command.args.float("position").unwrap_or_default();
            let Ok(position) = Duration::try_from_secs_f64(seconds) else {
                command.reply(Err(format!("cannot seek to {seconds} seconds")));
                return;
            };
            if !player.can_seek {
                command.reply(Err(format!("{} cannot seek", player.identity)));
                return;
            }
            Control::SeekTo(position)
        }
        other => {
            command.reply(Err(format!("unknown action {other}")));
            return;
        }
    };

    let connection = connection.clone();
    let bus = bus.to_owned();
    let player = player.clone();
    tokio::spawn(async move {
        let result = mpris::send(&connection, &bus, &player, control)
            .await
            .map_err(|error| format!("{}: {error}", player.identity));
        command.reply(result);
    });
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "media",
            include_str!("../settings.toml"),
        );
    }
}

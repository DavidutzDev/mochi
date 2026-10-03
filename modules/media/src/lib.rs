//! Now playing, from any player that speaks MPRIS: Spotify, browsers, mpv
//! with mpv-mpris, VLC and most others.
//!
//! While something plays, a compact view stays on the island at low
//! priority, so everything else interrupts it and it comes back afterwards.
//! A new track opens the expanded view for a few seconds, with the cover,
//! progress and controls. A pause shows briefly, then the island lets go.
//! Clicking the compact view expands it.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.media]
//! expand_ms = 4000   # how long a new track shows the expanded view
//! paused_ms = 3000   # how long a pause stays on the island
//! ignore = []        # players never shown, like ["firefox"]
//! ```

mod mpris;
mod notice;

use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, ArgSpec, Assets, BoxFuture, Module, ModuleCommand,
    ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use tokio::sync::mpsc;
use zbus::Connection;

use crate::mpris::{Control, Player, Status};
use crate::notice::{Notice, Tracker};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// One activity, replaced in place as playback changes.
const KEY: &str = "media";

#[derive(Debug, Default)]
pub struct Media;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    expand_ms: u64,
    paused_ms: u64,
    ignore: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            expand_ms: 4000,
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
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let connection = Connection::session().await?;

            let (sender, mut updates) = mpsc::unbounded_channel();
            tokio::spawn(mpris::watch(connection.clone(), sender));

            let mut tracker = Tracker::new(&settings.ignore);
            // The activity on the island or waiting for it, if any.
            let mut shown: Option<ActivityId> = None;
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            control(&connection, &tracker, command);
                        }
                        Some(ModuleEvent::Ended { activity, .. }) if shown == Some(activity) => {
                            shown = None;
                        }
                        Some(_) => {}
                    },
                    Some(update) = updates.recv() => {
                        let Some(notice) = tracker.apply(update) else { continue };
                        tracing::debug!(?notice, "media");
                        let player = tracker.chosen().map(|(_, player)| player);
                        match (notice, player) {
                            (Notice::Hide, _) | (_, None) => {
                                if let Some(id) = shown.take() {
                                    ctx.withdraw(id);
                                }
                            }
                            (Notice::Refresh, Some(player)) => {
                                if let Some(id) = shown {
                                    ctx.update(id, notice::payload(player));
                                }
                            }
                            (_, Some(player)) => {
                                let spec = spec(&settings, notice, player);
                                shown = Some(ctx.present(spec));
                            }
                        }
                    }
                }
            }
        })
    }
}

fn spec(settings: &Settings, notice: Notice, player: &Player) -> ActivitySpec {
    let mut spec = ActivitySpec::new("Compact")
        .expanded("Expanded")
        .key(KEY)
        .priority(Priority::LOW)
        .payload(notice::payload(player));
    if notice == Notice::Track {
        spec = spec.expand_for(Duration::from_millis(settings.expand_ms));
    }
    if player.status != Status::Playing {
        spec = spec.timeout(Duration::from_millis(settings.paused_ms));
    }
    spec
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

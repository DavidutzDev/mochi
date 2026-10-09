//! Picks the player to show and decides what each update means for the
//! island.
//!
//! The player shown is the most recently active one: the last to start
//! playing or change track while playing. A player that plays beats one that
//! doesn't, so a paused browser tab never takes the island from a playing
//! music player. A player picked with the arrows stays shown until it stops
//! or goes away, whatever the others do.

use std::collections::BTreeMap;
use std::time::SystemTime;

use serde_json::{Value, json};

use crate::mpris::{Player, Status, Update, short_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// A new track, or another player took over while playing.
    Track,
    /// Playback started or resumed.
    Playing,
    /// Playback paused.
    Paused,
    /// Same track and status. Details changed: art, position or controls.
    Refresh,
    /// Nothing to show any more.
    Hide,
    /// Another player was picked: shown where the last one was, without
    /// opening the island.
    Switched,
}

#[derive(Debug, Default)]
pub struct Tracker {
    /// Lowercase names of players never shown.
    ignore: Vec<String>,
    players: BTreeMap<String, Known>,
    /// Counts activations, so the latest has the highest stamp.
    clock: u64,
    shown: Option<Shown>,
    /// The player picked with the arrows, by bus name.
    pinned: Option<String>,
}

#[derive(Debug)]
struct Known {
    player: Player,
    /// When it last became active, from `clock`. 0 for never.
    active: u64,
}

/// What the island last heard about, to tell what changed.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    bus: String,
    track: TrackKey,
    status: Status,
}

/// Players fill in metadata over several updates; these fields together tell
/// one track from the next.
type TrackKey = (Option<String>, String, Vec<String>);

fn track_key(player: &Player) -> TrackKey {
    let track = &player.track;
    (track.id.clone(), track.title.clone(), track.artists.clone())
}

impl Tracker {
    /// `ignore` holds player names, like `firefox` or `Spotify`, matched
    /// without case against the bus name and the name the player gives
    /// itself.
    pub fn new(ignore: &[String]) -> Self {
        Self {
            ignore: ignore.iter().map(|name| name.to_lowercase()).collect(),
            ..Self::default()
        }
    }

    pub fn apply(&mut self, update: Update) -> Option<Notice> {
        match update {
            Update::Changed { bus, player } => {
                if self.ignored(&bus, &player) {
                    return None;
                }
                let previous = self.players.get(&bus);
                let started = player.status == Status::Playing
                    && previous.is_none_or(|known| {
                        known.player.status != Status::Playing
                            || track_key(&known.player) != track_key(&player)
                    });
                let active = if started {
                    self.clock += 1;
                    self.clock
                } else {
                    previous.map_or(0, |known| known.active)
                };
                self.players.insert(
                    bus,
                    Known {
                        player: *player,
                        active,
                    },
                );
            }
            Update::Gone { bus } => {
                self.players.remove(&bus)?;
            }
        }
        self.decide()
    }

    /// The player on the island, with its bus name.
    pub fn chosen(&self) -> Option<(&str, &Player)> {
        if let Some((bus, known)) = self
            .pinned
            .as_ref()
            .and_then(|bus| self.players.get_key_value(bus))
            .filter(|(_, known)| showable(&known.player))
        {
            return Some((bus.as_str(), &known.player));
        }
        self.players
            .iter()
            .filter(|(_, known)| showable(&known.player))
            .max_by_key(|(_, known)| (known.player.status == Status::Playing, known.active))
            .map(|(bus, known)| (bus.as_str(), &known.player))
    }

    /// The players the arrows go through, in bus name order.
    pub fn players(&self) -> impl Iterator<Item = (&str, &Player)> {
        self.players
            .iter()
            .filter(|(_, known)| showable(&known.player))
            .map(|(bus, known)| (bus.as_str(), &known.player))
    }

    /// Shows the player `step` places after the shown one, wrapping around.
    pub fn step(&mut self, step: isize) -> Result<Option<Notice>, String> {
        let current = self
            .chosen()
            .ok_or("no media player is playing anything")?
            .0;
        let players: Vec<&str> = self.players().map(|(bus, _)| bus).collect();
        let index = players
            .iter()
            .position(|bus| *bus == current)
            .unwrap_or_default();
        let count = players.len() as isize;
        let bus = players[(index as isize + step).rem_euclid(count) as usize].to_owned();
        Ok(self.pin(bus))
    }

    /// Shows the player with this name, like `firefox` or `Spotify`.
    pub fn pick(&mut self, name: &str) -> Result<Option<Notice>, String> {
        let name = name.to_lowercase();
        let bus = self
            .players()
            .find(|(bus, player)| {
                short_name(bus).to_lowercase() == name || player.identity.to_lowercase() == name
            })
            .ok_or_else(|| format!("no player called {name} has a track"))?
            .0
            .to_owned();
        Ok(self.pin(bus))
    }

    /// Keeps `bus` shown until it stops.
    fn pin(&mut self, bus: String) -> Option<Notice> {
        let changed = self.chosen().is_none_or(|(shown, _)| shown != bus);
        self.pinned = Some(bus);
        if !changed {
            return None;
        }
        self.shown = self.chosen().map(|(bus, player)| Shown {
            bus: bus.to_owned(),
            track: track_key(player),
            status: player.status,
        });
        Some(Notice::Switched)
    }

    /// What the views get: the shown player, and the players the arrows go
    /// through.
    pub fn payload(&self) -> Value {
        let Some((current, player)) = self.chosen() else {
            return Value::Null;
        };
        let mut payload = payload(player);
        let players: Vec<Value> = self
            .players()
            .map(|(bus, player)| {
                json!({
                    "name": player.identity,
                    "shown": bus == current,
                    "playing": player.status == Status::Playing,
                    "volume": volume(player),
                })
            })
            .collect();
        payload["players"] = Value::Array(players);
        payload
    }

    fn ignored(&self, bus: &str, player: &Player) -> bool {
        let short = short_name(bus).to_lowercase();
        let identity = player.identity.to_lowercase();
        self.ignore
            .iter()
            .any(|name| *name == short || *name == identity)
    }

    fn decide(&mut self) -> Option<Notice> {
        if let Some(bus) = &self.pinned
            && !self
                .players
                .get(bus)
                .is_some_and(|known| showable(&known.player))
        {
            self.pinned = None;
        }
        let next = self.chosen().map(|(bus, player)| Shown {
            bus: bus.to_owned(),
            track: track_key(player),
            status: player.status,
        });
        let previous = std::mem::replace(&mut self.shown, next.clone());
        let Some(next) = next else {
            return previous.map(|_| Notice::Hide);
        };
        let same_player = previous
            .as_ref()
            .is_some_and(|previous| previous.bus == next.bus);

        let notice = match previous {
            Some(previous) if same_player && previous.track == next.track => {
                if previous.status == next.status {
                    Notice::Refresh
                } else if next.status == Status::Playing {
                    Notice::Playing
                } else {
                    Notice::Paused
                }
            }
            // A paused player appearing or taking over has nothing new to
            // say.
            None if next.status != Status::Playing => return None,
            Some(_) if !same_player && next.status != Status::Playing => Notice::Hide,
            _ => Notice::Track,
        };
        Some(notice)
    }
}

/// Whether a player has something to show.
fn showable(player: &Player) -> bool {
    player.status != Status::Stopped && !player.track.title.is_empty()
}

/// What the views get. `position_ms` is the position at `read_at_ms`, so a
/// view can move it on by itself, even when the activity shows up later.
pub fn payload(player: &Player) -> Value {
    let track = &player.track;
    let millis = |duration: std::time::Duration| duration.as_millis() as u64;
    let read_at = player
        .read_at
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    json!({
        "player": player.identity,
        "status": player.status.as_str(),
        "title": track.title,
        "artist": track.artists.join(", "),
        "album": track.album,
        "art": track.art,
        "length_ms": track.length.map(millis),
        "position_ms": player.position.map(millis),
        "read_at_ms": millis(read_at),
        "rate": player.rate,
        "can_previous": player.can_previous,
        "can_next": player.can_next,
        "can_play_pause": player.can_play || player.can_pause,
        "can_seek": player.can_seek && track.length.is_some(),
        "volume": volume(player),
        "can_volume": player.volume.is_some() && player.can_control,
    })
}

/// The player's own volume in percent, or `None` when it doesn't report one.
pub fn volume(player: &Player) -> Option<u32> {
    player.volume.map(|volume| (volume * 100.0).round() as u32)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::mpris::Track;

    const SPOTIFY: &str = "org.mpris.MediaPlayer2.spotify";
    const FIREFOX: &str = "org.mpris.MediaPlayer2.firefox.instance_1_35";

    fn player(title: &str, status: Status) -> Player {
        Player {
            identity: "Player".into(),
            status,
            track: Track {
                title: title.into(),
                artists: vec!["Artist".into()],
                ..Track::default()
            },
            position: Some(Duration::ZERO),
            read_at: SystemTime::UNIX_EPOCH,
            rate: 1.0,
            can_previous: true,
            can_next: true,
            can_play: true,
            can_pause: true,
            can_seek: true,
            volume: None,
            can_control: true,
        }
    }

    fn changed(bus: &str, player: Player) -> Update {
        Update::Changed {
            bus: bus.into(),
            player: Box::new(player),
        }
    }

    fn chosen(tracker: &Tracker) -> Option<&str> {
        tracker.chosen().map(|(bus, _)| bus)
    }

    #[test]
    fn follows_one_player_through_a_session() {
        let mut tracker = Tracker::default();
        let song = |status| changed(SPOTIFY, player("One", status));

        assert_eq!(tracker.apply(song(Status::Playing)), Some(Notice::Track));
        assert_eq!(tracker.apply(song(Status::Paused)), Some(Notice::Paused));
        assert_eq!(tracker.apply(song(Status::Playing)), Some(Notice::Playing));
        assert_eq!(tracker.apply(song(Status::Playing)), Some(Notice::Refresh));
        assert_eq!(
            tracker.apply(changed(SPOTIFY, player("Two", Status::Playing))),
            Some(Notice::Track)
        );
        assert_eq!(
            tracker.apply(Update::Gone {
                bus: SPOTIFY.into()
            }),
            Some(Notice::Hide)
        );
        assert_eq!(chosen(&tracker), None);
    }

    #[test]
    fn a_new_track_while_paused_still_shows() {
        let mut tracker = Tracker::default();
        tracker.apply(changed(SPOTIFY, player("One", Status::Paused)));
        assert_eq!(
            tracker.apply(changed(SPOTIFY, player("Two", Status::Paused))),
            Some(Notice::Track)
        );
    }

    #[test]
    fn stopping_hides_the_player() {
        let mut tracker = Tracker::default();
        tracker.apply(changed(SPOTIFY, player("One", Status::Playing)));
        assert_eq!(
            tracker.apply(changed(SPOTIFY, player("One", Status::Stopped))),
            Some(Notice::Hide)
        );
        assert_eq!(
            tracker.apply(changed(SPOTIFY, player("One", Status::Stopped))),
            None
        );
    }

    #[test]
    fn the_most_recently_started_player_wins() {
        let mut tracker = Tracker::default();
        tracker.apply(changed(SPOTIFY, player("Song", Status::Playing)));
        assert_eq!(
            tracker.apply(changed(FIREFOX, player("Video", Status::Playing))),
            Some(Notice::Track)
        );
        assert_eq!(chosen(&tracker), Some(FIREFOX));

        // Pausing the video hands the island back to the music still playing.
        assert_eq!(
            tracker.apply(changed(FIREFOX, player("Video", Status::Paused))),
            Some(Notice::Track)
        );
        assert_eq!(chosen(&tracker), Some(SPOTIFY));
    }

    #[test]
    fn a_paused_player_never_takes_over() {
        let mut tracker = Tracker::default();
        tracker.apply(changed(SPOTIFY, player("Song", Status::Playing)));
        assert_eq!(
            tracker.apply(changed(FIREFOX, player("Video", Status::Paused))),
            Some(Notice::Refresh)
        );
        assert_eq!(chosen(&tracker), Some(SPOTIFY));

        // When the music quits, the paused tab is chosen but not shown.
        assert_eq!(
            tracker.apply(Update::Gone {
                bus: SPOTIFY.into()
            }),
            Some(Notice::Hide)
        );
        assert_eq!(chosen(&tracker), Some(FIREFOX));
        assert_eq!(
            tracker.apply(changed(FIREFOX, player("Video", Status::Playing))),
            Some(Notice::Playing)
        );
    }

    #[test]
    fn a_paused_player_appearing_stays_quiet() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.apply(changed(FIREFOX, player("Video", Status::Paused))),
            None
        );
        assert_eq!(chosen(&tracker), Some(FIREFOX));
    }

    #[test]
    fn players_without_a_track_are_not_shown() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.apply(changed(SPOTIFY, player("", Status::Playing))),
            None
        );
        assert_eq!(chosen(&tracker), None);
    }

    #[test]
    fn ignores_players_by_bus_name_or_identity() {
        let mut tracker = Tracker::new(&["Firefox".into(), "vlc media player".into()]);
        assert_eq!(
            tracker.apply(changed(FIREFOX, player("Video", Status::Playing))),
            None
        );
        let vlc = Player {
            identity: "VLC media player".into(),
            ..player("Song", Status::Playing)
        };
        assert_eq!(
            tracker.apply(changed("org.mpris.MediaPlayer2.vlc", vlc)),
            None
        );
        assert_eq!(chosen(&tracker), None);
    }

    #[test]
    fn a_picked_player_stays_until_it_stops() {
        let mut tracker = Tracker::default();
        tracker.apply(changed(SPOTIFY, player("Song", Status::Playing)));
        tracker.apply(changed(FIREFOX, player("Video", Status::Paused)));
        assert_eq!(chosen(&tracker), Some(SPOTIFY));

        assert_eq!(tracker.step(1), Ok(Some(Notice::Switched)));
        assert_eq!(chosen(&tracker), Some(FIREFOX));
        // Picking the shown one changes nothing.
        assert_eq!(tracker.pick("firefox"), Ok(None));

        // A new song doesn't take the island back.
        assert_eq!(
            tracker.apply(changed(SPOTIFY, player("Next song", Status::Playing))),
            Some(Notice::Refresh)
        );
        assert_eq!(chosen(&tracker), Some(FIREFOX));

        // Until the video stops.
        assert_eq!(
            tracker.apply(changed(FIREFOX, player("Video", Status::Stopped))),
            Some(Notice::Track)
        );
        assert_eq!(chosen(&tracker), Some(SPOTIFY));
        tracker.apply(changed(FIREFOX, player("Video", Status::Playing)));
        assert_eq!(chosen(&tracker), Some(FIREFOX));
        assert_eq!(
            tracker.apply(changed(SPOTIFY, player("Third song", Status::Playing))),
            Some(Notice::Track)
        );
        assert_eq!(chosen(&tracker), Some(SPOTIFY));
    }

    #[test]
    fn the_arrows_wrap_around() {
        let mut tracker = Tracker::default();
        assert!(tracker.step(1).is_err());
        tracker.apply(changed(SPOTIFY, player("Song", Status::Playing)));
        assert_eq!(tracker.step(1), Ok(None));
        tracker.apply(changed(FIREFOX, player("Video", Status::Paused)));

        // Firefox sorts first, so the previous one wraps to it.
        assert_eq!(tracker.step(-1), Ok(Some(Notice::Switched)));
        assert_eq!(chosen(&tracker), Some(FIREFOX));
        assert_eq!(tracker.step(-1), Ok(Some(Notice::Switched)));
        assert_eq!(chosen(&tracker), Some(SPOTIFY));
        assert!(tracker.pick("vlc").is_err());

        let payload = tracker.payload();
        assert_eq!(payload["players"].as_array().map(Vec::len), Some(2));
        assert_eq!(payload["players"][1]["shown"], true);
        assert_eq!(payload["players"][1]["playing"], true);
    }

    #[test]
    fn payload_carries_what_the_views_need() {
        let mut player = player("Song", Status::Playing);
        player.track.length = Some(Duration::from_secs(90));
        player.read_at = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        let payload = payload(&player);
        assert_eq!(payload["title"], "Song");
        assert_eq!(payload["status"], "playing");
        assert_eq!(payload["length_ms"], 90_000);
        assert_eq!(payload["read_at_ms"], 2_000);
        assert_eq!(payload["art"], Value::Null);
        assert_eq!(payload["can_seek"], true);
        // Without a volume of its own, the views show no slider.
        assert_eq!(payload["volume"], Value::Null);
        assert_eq!(payload["can_volume"], false);
    }

    #[test]
    fn payload_carries_the_players_volume() {
        let mut tracker = Tracker::default();
        let spotify = Player {
            volume: Some(0.348),
            ..player("Song", Status::Playing)
        };
        tracker.apply(changed(SPOTIFY, spotify.clone()));
        tracker.apply(changed(FIREFOX, player("Video", Status::Paused)));
        let payload = tracker.payload();
        assert_eq!(payload["volume"], 35);
        assert_eq!(payload["can_volume"], true);
        assert_eq!(payload["players"][0]["volume"], Value::Null);
        assert_eq!(payload["players"][1]["volume"], 35);

        // A new volume is a refresh, not a new track.
        let louder = Player {
            volume: Some(0.5),
            ..spotify
        };
        assert_eq!(
            tracker.apply(changed(SPOTIFY, louder.clone())),
            Some(Notice::Refresh)
        );
        assert_eq!(tracker.payload()["volume"], 50);

        // A player that takes no commands reports its volume, but can't
        // have it changed.
        let fixed = Player {
            can_control: false,
            ..louder
        };
        tracker.apply(changed(SPOTIFY, fixed));
        assert_eq!(tracker.payload()["volume"], 50);
        assert_eq!(tracker.payload()["can_volume"], false);
    }
}

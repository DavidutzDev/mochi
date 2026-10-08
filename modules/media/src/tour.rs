//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "player", "Expanded", "Now playing")
            .icon("music_note")
            .order(30)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.1",
                "caption": "What's playing, from any player: it opens when a track starts, then folds into a bubble.",
                "payload": {
                    "player": "Spotify",
                    "status": "playing",
                    "title": "Teardrop",
                    "artist": "Massive Attack",
                    "album": "Mezzanine",
                    "art": "",
                    "length_ms": 330000,
                    "position_ms": 61000,
                    "read_at_ms": 0,
                    "rate": 0.0,
                    "can_previous": true,
                    "can_next": true,
                    "can_play_pause": true,
                    "can_seek": true,
                    "players": [
                        {
                            "name": "Spotify",
                            "shown": true,
                            "playing": true
                        }
                    ]
                }
            })),
        ContributionSpec::new("tour", "step", "bubble", "BubbleWide", "Music bubble")
            .icon("music_note")
            .order(31)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.1",
                "place": "bubble",
                "caption": "The music stays at hand in a bubble; a click brings the player back.",
                "payload": {
                    "player": "Spotify",
                    "status": "playing",
                    "title": "Teardrop",
                    "artist": "Massive Attack",
                    "album": "Mezzanine",
                    "art": "",
                    "length_ms": 330000,
                    "position_ms": 61000,
                    "read_at_ms": 0,
                    "rate": 0.0,
                    "can_previous": true,
                    "can_next": true,
                    "can_play_pause": true,
                    "can_seek": true,
                    "players": [
                        {
                            "name": "Spotify",
                            "shown": true,
                            "playing": true
                        }
                    ],
                    "area": "center-left"
                }
            })),
    ]
}

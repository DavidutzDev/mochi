//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.
//!
//! The made-up reminder and forecast start from now, so the panel, which
//! shows the real time, agrees with them.

use mochi_core::ContributionSpec;
use serde_json::{Value, json};

use crate::local;

pub fn steps() -> Vec<ContributionSpec> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);
    let here = local::from_epoch(now);
    // Two days from now at half past seven in the evening.
    let dinner = local::from_epoch(now + 2 * 86_400)
        .and_then(|later| local::to_epoch(later.date, 19, 30))
        .unwrap_or(now);
    vec![
        ContributionSpec::new("tour", "step", "panel", "Panel", "The clock")
            .icon("schedule")
            .order(25)
            .options(json!({
                "chapter": "panels",
                "since": "0.1.0",
                "caption": "The clock: today with the weather, a calendar with reminders, the focus timer, a stopwatch and world clocks. Bind mochi ipc clock toggle to a key.",
                "payload": {
                    "tab": "today",
                    "state": {
                        "tab": "today",
                        "hours": "24",
                        "first_day": "monday",
                        "world": ["Europe/London", "America/New_York", "Asia/Tokyo"],
                        "zones": {},
                        "unknownZones": [],
                        "reminders": [
                            { "id": 1, "at": dinner, "text": "Ana's birthday dinner", "done": false }
                        ],
                        "stopwatch": { "since_ms": null, "banked_ms": 0, "laps": [] }
                    },
                    "weather": weather(
                        here.map_or(14, |here| here.hour),
                        here.map_or(0, |here| here.weekday),
                    ),
                }
            })),
        ContributionSpec::new("tour", "step", "reminder", "Notice", "Reminders")
            .icon("notifications")
            .order(48)
            .options(json!({
                "chapter": "notices",
                "since": "0.1.0",
                "caption": "A reminder from the clock's calendar, when it's due, also if the computer slept through it. Done, or snooze it for 10 minutes.",
                // No time: it's due now.
                "payload": {
                    "id": 1,
                    "text": "Water the plants",
                    "hours": "24"
                }
            })),
    ]
}

/// A made-up day in Lyon from `hour` on `weekday`, in the shape of the
/// weather module's state.
fn weather(hour: u32, weekday: u32) -> Value {
    let hourly: Vec<Value> = [
        (18.0, "partly_cloudy_day"),
        (19.0, "partly_cloudy_day"),
        (19.0, "clear_day"),
        (18.0, "clear_day"),
        (16.0, "partly_cloudy_night"),
        (14.0, "clear_night"),
        (13.0, "clear_night"),
        (12.0, "clear_night"),
    ]
    .iter()
    .zip(hour..)
    .map(|((temperature, icon), hour)| {
        json!({ "hour": hour % 24, "temperature": temperature, "icon": icon })
    })
    .collect();
    let daily: Vec<Value> = [
        (9.0, 19.0, "partly_cloudy_day"),
        (10.0, 21.0, "clear_day"),
        (11.0, 17.0, "rainy"),
        (8.0, 15.0, "rainy"),
        (7.0, 16.0, "cloud"),
    ]
    .iter()
    .zip(weekday..)
    .map(|((min, max, icon), weekday)| {
        json!({ "weekday": weekday % 7, "min": min, "max": max, "icon": icon })
    })
    .collect();
    json!({
        "configured": true,
        "loading": false,
        "error": null,
        "unit": { "temperature": "°C", "speed": "km/h" },
        "place": { "name": "Lyon" },
        "current": {
            "temperature": 18.4,
            "feels_like": 17.0,
            "humidity": 54,
            "wind_speed": 12.0,
            "uv_index": 3.2,
            "text": "Partly cloudy",
            "icon": "partly_cloudy_day",
        },
        "hourly": hourly,
        "daily": daily,
    })
}

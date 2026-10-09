//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::{Value, json};

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "card", "Card", "Weather")
            .icon("partly_cloudy_day")
            .order(56)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.10",
                "place": "card",
                "size": [548, 96],
                "caption": "The weather where you are, from Open-Meteo, with the next hours. Nothing is sent until you set a place in its settings.",
                "payload": payload(),
            })),
        ContributionSpec::new("tour", "step", "page", "Page", "Weather")
            .icon("partly_cloudy_day")
            .order(57)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.10",
                "place": "card",
                "size": [828, 470],
                "caption": "Its page in the control center: the weather now with the wind, the UV index and the sun, the next 7 days and the next 24 hours.",
                "payload": payload(),
            })),
    ]
}

/// The made-up day's offset from UTC, as in Lyon in October.
const OFFSET: i64 = 7200;
/// Midnight in Lyon on the made-up day, Friday 9 October 2026.
const MIDNIGHT: i64 = 1_791_504_000 - OFFSET;

/// A made-up afternoon in Lyon, in the shape of the module's state.
fn payload() -> Value {
    let sky = |code: i64, text: &str, icon: &str, day: bool| json!({ "code": code, "text": text, "icon": icon, "day": day });
    let mut hourly = Vec::new();
    for (index, (temperature, code, text, icon)) in [
        (18.0, 2, "Partly cloudy", "partly_cloudy_day"),
        (19.0, 1, "Mostly clear", "partly_cloudy_day"),
        (19.0, 0, "Clear", "clear_day"),
        (17.0, 3, "Overcast", "cloud"),
        (15.0, 61, "Light rain", "rainy_light"),
        (14.0, 63, "Rain", "rainy"),
        (13.0, 3, "Overcast", "cloud"),
        (13.0, 3, "Overcast", "cloud"),
        (12.0, 2, "Partly cloudy", "partly_cloudy_night"),
        (12.0, 1, "Mostly clear", "partly_cloudy_night"),
        (11.0, 0, "Clear", "clear_night"),
        (11.0, 0, "Clear", "clear_night"),
        (10.0, 0, "Clear", "clear_night"),
        (10.0, 0, "Clear", "clear_night"),
        (9.0, 0, "Clear", "clear_night"),
        (9.0, 1, "Mostly clear", "partly_cloudy_night"),
        (9.0, 1, "Mostly clear", "partly_cloudy_night"),
        (10.0, 2, "Partly cloudy", "partly_cloudy_day"),
        (11.0, 2, "Partly cloudy", "partly_cloudy_day"),
        (13.0, 1, "Mostly clear", "partly_cloudy_day"),
        (15.0, 0, "Clear", "clear_day"),
        (16.0, 0, "Clear", "clear_day"),
        (17.0, 0, "Clear", "clear_day"),
        (18.0, 1, "Mostly clear", "partly_cloudy_day"),
    ]
    .into_iter()
    .enumerate()
    {
        let hour_of_day = (14 + index) % 24;
        let mut hour = sky(code, text, icon, (8..19).contains(&hour_of_day));
        hour["hour"] = json!(hour_of_day);
        hour["temperature"] = json!(temperature);
        hourly.push(hour);
    }
    let mut daily = Vec::new();
    for (index, (min, max, code, text, icon, precipitation)) in [
        (11.0, 19.0, 61, "Light rain", "rainy_light", 60),
        (9.0, 18.0, 0, "Clear", "clear_day", 0),
        (10.0, 20.0, 2, "Partly cloudy", "partly_cloudy_day", 10),
        (12.0, 21.0, 3, "Overcast", "cloud", 20),
        (13.0, 17.0, 63, "Rain", "rainy", 80),
        (10.0, 15.0, 61, "Light rain", "rainy_light", 50),
        (8.0, 16.0, 1, "Mostly clear", "partly_cloudy_day", 0),
    ]
    .into_iter()
    .enumerate()
    {
        let midnight = MIDNIGHT + index as i64 * 86_400;
        let mut day = sky(code, text, icon, true);
        day["time"] = json!(midnight);
        // From Friday, where 0 is Sunday.
        day["weekday"] = json!((5 + index) % 7);
        day["min"] = json!(min);
        day["max"] = json!(max);
        day["precipitation"] = json!(precipitation);
        day["sunrise"] = json!(midnight + 7 * 3600 + 48 * 60);
        day["sunset"] = json!(midnight + 19 * 3600 + 12 * 60);
        daily.push(day);
    }
    let mut current = sky(2, "Partly cloudy", "partly_cloudy_day", true);
    current["temperature"] = json!(18.4);
    current["feels_like"] = json!(17.0);
    current["humidity"] = json!(64);
    current["wind_speed"] = json!(14.0);
    current["wind_direction"] = json!(320);
    current["uv_index"] = json!(3.2);
    json!({
        "configured": true,
        "loading": false,
        "error": null,
        "units": "metric",
        "unit": { "temperature": "°C", "speed": "km/h" },
        "place": { "name": "Lyon", "region": "", "country": "France" },
        "utc_offset": OFFSET,
        "current": current,
        "hourly": hourly,
        "daily": daily,
    })
}

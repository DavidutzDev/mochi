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
    ]
}

/// A made-up afternoon in Lyon, in the shape of the module's state.
fn payload() -> Value {
    let sky = |code: i64, text: &str, icon: &str| json!({ "code": code, "text": text, "icon": icon, "day": true });
    let mut hourly = Vec::new();
    for (index, (temperature, code, text, icon)) in [
        (18.0, 2, "Partly cloudy", "partly_cloudy_day"),
        (19.0, 1, "Mostly clear", "partly_cloudy_day"),
        (19.0, 0, "Clear", "clear_day"),
        (17.0, 3, "Overcast", "cloud"),
        (15.0, 61, "Light rain", "rainy_light"),
        (14.0, 63, "Rain", "rainy"),
        (13.0, 3, "Overcast", "cloud"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut hour = sky(code, text, icon);
        hour["hour"] = json!(14 + index);
        hour["temperature"] = json!(temperature);
        hourly.push(hour);
    }
    let mut current = sky(2, "Partly cloudy", "partly_cloudy_day");
    current["temperature"] = json!(18.4);
    current["feels_like"] = json!(17.0);
    json!({
        "configured": true,
        "loading": false,
        "error": null,
        "units": "metric",
        "unit": { "temperature": "°C", "speed": "km/h" },
        "place": { "name": "Lyon", "region": "", "country": "France" },
        "current": current,
        "hourly": hourly,
        "daily": [{ "min": 11.0, "max": 19.0, "code": 61, "text": "Light rain", "icon": "rainy_light" }],
    })
}

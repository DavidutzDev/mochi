//! An example Mochi plugin: the weather from Open-Meteo.
//!
//! It looks the city up once, then fetches the current weather and three days
//! of forecast every `refresh_minutes`, and publishes them for its control
//! center card. `show` puts the forecast on the island. It fetches with `curl`,
//! to stay small; a plugin of your own can use any HTTP client.

use std::time::Duration;

use mochi_sdk::{
    ActivitySpec, Area, BubbleId, BubbleSpec, ModuleCommand, ModuleCtx, ModuleEvent, Value, json,
};
use serde::Deserialize;
use tokio::time::{Instant, sleep_until};

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    city: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
    units: Units,
    refresh_minutes: u64,
    bubble: Option<Area>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            city: String::new(),
            latitude: None,
            longitude: None,
            units: Units::Metric,
            refresh_minutes: 20,
            bubble: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Units {
    Metric,
    Imperial,
}

#[derive(Debug, Clone)]
struct Place {
    name: String,
    latitude: f64,
    longitude: f64,
}

fn main() -> std::process::ExitCode {
    mochi_sdk::run(run)
}

async fn run(mut ctx: ModuleCtx) -> Result<(), mochi_sdk::Error> {
    let settings: Settings = ctx.settings()?;
    let every = Duration::from_secs(settings.refresh_minutes.max(5) * 60);
    let mut place: Option<Place> = None;
    let mut weather = Value::Null;
    let mut bubble: Option<BubbleId> = None;
    let mut next = Instant::now();

    loop {
        tokio::select! {
            event = ctx.next_event() => match event {
                None => return Ok(()),
                Some(ModuleEvent::Command(command)) => match command.action.as_str() {
                    "refresh" => {
                        next = Instant::now();
                        command.reply(Ok(()));
                    }
                    "show" => show(&ctx, &weather, command),
                    _ => command.reply(Err("no such action".into())),
                },
                Some(ModuleEvent::BubbleClicked(_)) => {
                    ctx.present(forecast(&weather));
                }
                Some(_) => {}
            },
            () = sleep_until(next) => {
                next = Instant::now() + every;
                if place.is_none() {
                    match locate(&settings).await {
                        Ok(found) => place = Some(found),
                        Err(error) => {
                            ctx.publish_state(json!({ "error": error }));
                            // Try again sooner than a normal refresh.
                            next = Instant::now() + Duration::from_secs(60);
                            continue;
                        }
                    }
                }
                let Some(place) = &place else { continue };
                weather = match fetch(place, settings.units).await {
                    Ok(weather) => weather,
                    Err(error) => {
                        eprintln!("weather: {error}");
                        json!({ "place": place.name, "error": error })
                    }
                };
                ctx.publish_state(weather.clone());
                if let Some(area) = settings.bubble
                    && weather.get("temperature").is_some()
                {
                    let spec = BubbleSpec::new("Bubble").key("now").area(area).payload(weather.clone());
                    match bubble {
                        Some(id) => ctx.update_bubble(id, weather.clone()),
                        None => bubble = Some(ctx.show_bubble(spec)),
                    }
                }
            }
        }
    }
}

fn show(ctx: &ModuleCtx, weather: &Value, command: ModuleCommand) {
    if weather.get("temperature").is_none() {
        let reason = weather["error"].as_str().unwrap_or("no weather yet");
        command.reply(Err(reason.to_owned()));
        return;
    }
    ctx.present(forecast(weather));
    command.reply(Ok(()));
}

fn forecast(weather: &Value) -> ActivitySpec {
    ActivitySpec::new("Forecast")
        .key("forecast")
        .timeout(Duration::from_secs(8))
        .payload(weather.clone())
}

/// Where to fetch the weather for: the coordinates in the settings, or the
/// city looked up.
async fn locate(settings: &Settings) -> Result<Place, String> {
    if let (Some(latitude), Some(longitude)) = (settings.latitude, settings.longitude) {
        let name = if settings.city.is_empty() {
            format!("{latitude:.2}, {longitude:.2}")
        } else {
            settings.city.clone()
        };
        return Ok(Place {
            name,
            latitude,
            longitude,
        });
    }
    if settings.city.is_empty() {
        return Err("set `city`, or `latitude` and `longitude`, in [module.weather]".into());
    }
    // "Lyon, France": look up the name, prefer the country.
    let (name, country) = match settings.city.split_once(',') {
        Some((name, country)) => (name.trim(), Some(country.trim().to_lowercase())),
        None => (settings.city.trim(), None),
    };
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?count=10&format=json&name={}",
        encode(name)
    );
    let found = get(&url).await?;
    let results = found["results"].as_array().cloned().unwrap_or_default();
    let matches = |result: &&Value| {
        country.as_deref().is_none_or(|country| {
            result["country"].as_str().map(str::to_lowercase).as_deref() == Some(country)
                || result["country_code"]
                    .as_str()
                    .map(str::to_lowercase)
                    .as_deref()
                    == Some(country)
        })
    };
    let result = results
        .iter()
        .find(matches)
        .ok_or_else(|| format!("Open-Meteo knows no place called {:?}", settings.city))?;
    Ok(Place {
        name: result["name"].as_str().unwrap_or(name).to_owned(),
        latitude: result["latitude"].as_f64().unwrap_or_default(),
        longitude: result["longitude"].as_f64().unwrap_or_default(),
    })
}

/// The current weather and three days, in the shape the views read.
async fn fetch(place: &Place, units: Units) -> Result<Value, String> {
    let (temperature, wind) = match units {
        Units::Metric => ("celsius", "kmh"),
        Units::Imperial => ("fahrenheit", "mph"),
    };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}\
         &current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,is_day\
         &daily=weather_code,temperature_2m_max,temperature_2m_min\
         &timezone=auto&forecast_days=3&temperature_unit={temperature}&wind_speed_unit={wind}",
        place.latitude, place.longitude
    );
    let data = get(&url).await?;
    let current = &data["current"];
    let code = current["weather_code"].as_i64().unwrap_or(-1);
    let daily = &data["daily"];
    let days: Vec<Value> = (0..3)
        .filter_map(|index| {
            let date = daily["time"][index].as_str()?;
            Some(json!({
                "date": date,
                "code": daily["weather_code"][index],
                "kind": kind(daily["weather_code"][index].as_i64().unwrap_or(-1)),
                "max": daily["temperature_2m_max"][index].as_f64()?.round(),
                "min": daily["temperature_2m_min"][index].as_f64()?.round(),
            }))
        })
        .collect();
    Ok(json!({
        "place": place.name,
        "temperature": current["temperature_2m"].as_f64().map(f64::round),
        "feels_like": current["apparent_temperature"].as_f64().map(f64::round),
        "wind": current["wind_speed_10m"].as_f64().map(f64::round),
        "code": code,
        "kind": kind(code),
        "description": describe(code),
        "day": current["is_day"].as_i64() != Some(0),
        "unit": match units { Units::Metric => "°C", Units::Imperial => "°F" },
        "wind_unit": wind.replace("kmh", "km/h"),
        "updated": current["time"].as_str().and_then(|time| time.split('T').nth(1)),
        "days": days,
    }))
}

/// Which icon a WMO weather code gets.
fn kind(code: i64) -> &'static str {
    match code {
        0 | 1 => "clear",
        2 | 3 => "cloudy",
        45 | 48 => "fog",
        51..=67 | 80..=82 => "rain",
        71..=77 | 85 | 86 => "snow",
        95..=99 => "storm",
        _ => "cloudy",
    }
}

fn describe(code: i64) -> &'static str {
    match code {
        0 => "Clear",
        1 => "Mostly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51..=55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 => "Rain",
        65 => "Heavy rain",
        66 | 67 => "Freezing rain",
        71 | 73 => "Snow",
        75 => "Heavy snow",
        77 => "Snow grains",
        80..=82 => "Showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96..=99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
}

async fn get(url: &str) -> Result<Value, String> {
    let output = tokio::process::Command::new("curl")
        .args(["-fsS", "--max-time", "15", url])
        .output()
        .await
        .map_err(|error| format!("cannot run curl: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Open-Meteo didn't answer: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("Open-Meteo's answer: {error}"))
}

/// Percent-encodes a query value.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_encoded() {
        assert_eq!(encode("Saint-Étienne"), "Saint-%C3%89tienne");
        assert_eq!(encode("New York"), "New%20York");
    }

    #[test]
    fn codes_have_kinds() {
        assert_eq!(kind(0), "clear");
        assert_eq!(kind(63), "rain");
        assert_eq!(kind(75), "snow");
        assert_eq!(kind(96), "storm");
    }
}

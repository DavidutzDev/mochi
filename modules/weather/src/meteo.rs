//! Open-Meteo, which needs no account or key: the forecast API for the
//! weather at a place, and the geocoding API for where a place's name is.
//!
//! Requests go through a [`Fetch`], curl in Mochi and JSON files in the
//! tests, so the parsing here never needs the network to be checked.

use std::time::Duration;

use mochi_core::BoxFuture;
use serde::{Deserialize, Serialize};

pub const FORECAST: &str = "https://api.open-meteo.com/v1/forecast";
pub const GEOCODING: &str = "https://geocoding-api.open-meteo.com/v1/search";

/// How many days and hours of forecast a request asks for.
pub const DAYS: usize = 7;
pub const HOURS: usize = 24;

/// Gets a URL's body, or says why it can't.
pub trait Fetch: Send + Sync {
    fn get(&self, url: String) -> BoxFuture<'static, Result<String, String>>;
}

/// Fetches with curl: https only, with a time limit, and killed if the
/// module stops meanwhile.
#[derive(Debug)]
pub struct Curl;

impl Fetch for Curl {
    fn get(&self, url: String) -> BoxFuture<'static, Result<String, String>> {
        Box::pin(curl(url))
    }
}

async fn curl(url: String) -> Result<String, String> {
    let run = tokio::process::Command::new("curl")
        .args([
            "-sS",
            "--fail-with-body",
            "--proto",
            "=https",
            "--max-time",
            "20",
            &url,
        ])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(30), run)
        .await
        .map_err(|_| "Open-Meteo didn't answer in time".to_owned())?
        .map_err(|error| format!("can't run curl: {error}"))?;
    let body = String::from_utf8(output.stdout).map_err(|_| "Open-Meteo's answer isn't text")?;
    if output.status.success() {
        return Ok(body);
    }
    // Open-Meteo says what was wrong with a request in its body.
    if let Some(reason) = refusal(&body) {
        return Err(format!("Open-Meteo says: {reason}"));
    }
    let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if error.is_empty() {
        "can't reach Open-Meteo".to_owned()
    } else {
        format!(
            "can't reach Open-Meteo: {}",
            error.trim_start_matches("curl: ")
        )
    })
}

/// The reason in an answer like `{"error": true, "reason": "..."}`.
fn refusal(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Refusal {
        error: bool,
        reason: String,
    }
    serde_json::from_str::<Refusal>(body)
        .ok()
        .filter(|refusal| refusal.error)
        .map(|refusal| refusal.reason)
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    /// °C and km/h.
    #[default]
    Metric,
    /// °F and mph.
    Imperial,
}

impl Units {
    pub fn name(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
        }
    }

    pub fn temperature(self) -> &'static str {
        match self {
            Self::Metric => "°C",
            Self::Imperial => "°F",
        }
    }

    pub fn speed(self) -> &'static str {
        match self {
            Self::Metric => "km/h",
            Self::Imperial => "mph",
        }
    }
}

/// Where the weather is for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub name: String,
    /// The state or region, when the lookup gave one.
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// The lookup of `name`.
pub fn geocoding_url(name: &str) -> String {
    format!("{GEOCODING}?count=10&format=json&name={}", encode(name))
}

/// The best match for `query` in a geocoding answer. A query like "Lyon,
/// France" or "Springfield, Illinois" looks up the name before the comma
/// and keeps the results whose country, its code or a region match the
/// rest.
pub fn find_place(json: &str, query: &str) -> Result<Place, String> {
    #[derive(Deserialize)]
    struct Answer {
        #[serde(default)]
        results: Vec<Found>,
    }
    #[derive(Deserialize)]
    struct Found {
        name: String,
        latitude: f64,
        longitude: f64,
        #[serde(default)]
        country: String,
        #[serde(default)]
        country_code: String,
        #[serde(default)]
        admin1: String,
        #[serde(default)]
        admin2: String,
    }
    let answer: Answer = serde_json::from_str(json)
        .map_err(|error| format!("Open-Meteo's lookup isn't what it should be: {error}"))?;
    let within = query
        .split_once(',')
        .map(|(_, rest)| rest.trim().to_lowercase())
        .filter(|rest| !rest.is_empty());
    let found = answer
        .results
        .into_iter()
        .find(|found| {
            within.as_deref().is_none_or(|within| {
                [
                    &found.country,
                    &found.country_code,
                    &found.admin1,
                    &found.admin2,
                ]
                .iter()
                .any(|part| part.to_lowercase() == within)
            })
        })
        .ok_or_else(|| format!("Open-Meteo knows no place called {:?}", query.trim()))?;
    Ok(Place {
        name: found.name,
        region: found.admin1,
        country: found.country,
        latitude: found.latitude,
        longitude: found.longitude,
    })
}

/// The name to look up in a query like "Lyon, France".
pub fn looked_up(query: &str) -> &str {
    query.split(',').next().unwrap_or_default().trim()
}

/// The forecast for a place: now, the next hours and the next days, with
/// times in seconds since the epoch.
pub fn forecast_url(latitude: f64, longitude: f64, units: Units) -> String {
    let mut url = format!(
        "{FORECAST}?latitude={latitude}&longitude={longitude}\
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,\
         wind_speed_10m,wind_direction_10m,uv_index\
         &hourly=temperature_2m,weather_code,is_day,precipitation_probability\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,\
         precipitation_probability_max,uv_index_max\
         &timezone=auto&timeformat=unixtime&forecast_days={DAYS}&forecast_hours={HOURS}"
    );
    if units == Units::Imperial {
        url.push_str("&temperature_unit=fahrenheit&wind_speed_unit=mph");
    }
    url
}

/// A forecast, as the module keeps it. Times are seconds since the epoch;
/// `utc_offset` turns them into the place's own time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Forecast {
    /// The place's time zone, like Europe/Paris.
    pub timezone: String,
    /// Seconds from UTC at the place.
    pub utc_offset: i64,
    pub current: Current,
    pub hourly: Vec<Hour>,
    pub daily: Vec<Day>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Current {
    pub time: i64,
    pub temperature: f64,
    pub feels_like: Option<f64>,
    /// Relative humidity, in percent.
    pub humidity: Option<f64>,
    pub wind_speed: Option<f64>,
    /// Where the wind comes from, in degrees: 0 is north, 90 east.
    pub wind_direction: Option<f64>,
    pub uv_index: Option<f64>,
    pub code: i64,
    pub day: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hour {
    /// When the hour starts.
    pub time: i64,
    pub temperature: f64,
    pub code: i64,
    pub day: bool,
    /// The chance of rain or snow, in percent.
    pub precipitation: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Day {
    /// Midnight at the place.
    pub time: i64,
    pub code: i64,
    pub min: f64,
    pub max: f64,
    pub sunrise: Option<i64>,
    pub sunset: Option<i64>,
    pub precipitation: Option<f64>,
    pub uv_index: Option<f64>,
}

/// What the forecast API answers; the rest is left out. Values can be
/// `null` where a model has no data.
#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    timezone: String,
    #[serde(default)]
    utc_offset_seconds: i64,
    current: RawCurrent,
    hourly: RawHourly,
    daily: RawDaily,
}

#[derive(Deserialize)]
struct RawCurrent {
    time: i64,
    temperature_2m: Option<f64>,
    apparent_temperature: Option<f64>,
    relative_humidity_2m: Option<f64>,
    weather_code: Option<i64>,
    is_day: Option<i64>,
    wind_speed_10m: Option<f64>,
    wind_direction_10m: Option<f64>,
    uv_index: Option<f64>,
}

#[derive(Deserialize)]
struct RawHourly {
    time: Vec<i64>,
    temperature_2m: Vec<Option<f64>>,
    weather_code: Vec<Option<i64>>,
    is_day: Vec<Option<i64>>,
    #[serde(default)]
    precipitation_probability: Vec<Option<f64>>,
}

#[derive(Deserialize)]
struct RawDaily {
    time: Vec<i64>,
    weather_code: Vec<Option<i64>>,
    temperature_2m_max: Vec<Option<f64>>,
    temperature_2m_min: Vec<Option<f64>>,
    #[serde(default)]
    sunrise: Vec<Option<i64>>,
    #[serde(default)]
    sunset: Vec<Option<i64>>,
    #[serde(default)]
    precipitation_probability_max: Vec<Option<f64>>,
    #[serde(default)]
    uv_index_max: Vec<Option<f64>>,
}

/// The `index`th value of a column, when it's there and not `null`.
fn at<T: Copy>(column: &[Option<T>], index: usize) -> Option<T> {
    column.get(index).copied().flatten()
}

/// A forecast API answer, made into a [`Forecast`]. Hours and days with a
/// value missing are left out.
pub fn parse_forecast(json: &str) -> Result<Forecast, String> {
    if let Some(reason) = refusal(json) {
        return Err(format!("Open-Meteo says: {reason}"));
    }
    let raw: Raw = serde_json::from_str(json)
        .map_err(|error| format!("Open-Meteo's forecast isn't what it should be: {error}"))?;
    let now = &raw.current;
    let current = Current {
        time: now.time,
        temperature: now
            .temperature_2m
            .ok_or("Open-Meteo has no temperature for this place")?,
        feels_like: now.apparent_temperature,
        humidity: now.relative_humidity_2m,
        wind_speed: now.wind_speed_10m,
        wind_direction: now.wind_direction_10m,
        uv_index: now.uv_index,
        code: now.weather_code.unwrap_or(-1),
        day: now.is_day != Some(0),
    };
    let hours = &raw.hourly;
    let hourly = (0..hours.time.len())
        .filter_map(|index| {
            Some(Hour {
                time: hours.time[index],
                temperature: at(&hours.temperature_2m, index)?,
                code: at(&hours.weather_code, index)?,
                day: at(&hours.is_day, index) != Some(0),
                precipitation: at(&hours.precipitation_probability, index),
            })
        })
        .collect();
    let days = &raw.daily;
    let daily = (0..days.time.len())
        .filter_map(|index| {
            Some(Day {
                time: days.time[index],
                code: at(&days.weather_code, index)?,
                min: at(&days.temperature_2m_min, index)?,
                max: at(&days.temperature_2m_max, index)?,
                sunrise: at(&days.sunrise, index),
                sunset: at(&days.sunset, index),
                precipitation: at(&days.precipitation_probability_max, index),
                uv_index: at(&days.uv_index_max, index),
            })
        })
        .collect();
    Ok(Forecast {
        timezone: raw.timezone,
        utc_offset: raw.utc_offset_seconds,
        current,
        hourly,
        daily,
    })
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

/// The date of a day number since 1970-01-01, as (year, month, day).
pub fn civil(days: i64) -> (i64, u32, u32) {
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::fixtures::{FORECAST as FORECAST_JSON, GEOCODING as GEOCODING_JSON};

    #[test]
    fn a_forecast_parses() {
        let forecast = parse_forecast(FORECAST_JSON).unwrap();
        assert_eq!(forecast.timezone, "Europe/Paris");
        assert_eq!(forecast.utc_offset, 7200);
        let now = &forecast.current;
        assert_eq!(now.temperature, 14.8);
        assert_eq!(now.feels_like, Some(12.9));
        assert_eq!(now.humidity, Some(52.0));
        assert_eq!(now.code, 3);
        assert!(!now.day);
        assert_eq!(forecast.hourly.len(), HOURS);
        assert_eq!(forecast.hourly[0].time, 1_791_568_800);
        assert!(forecast.hourly[1].time - forecast.hourly[0].time == 3600);
        assert_eq!(forecast.daily.len(), DAYS);
        let today = &forecast.daily[0];
        assert_eq!((today.min, today.max), (9.8, 16.3));
        assert!(today.sunrise.unwrap() < today.sunset.unwrap());
        assert_eq!(forecast.daily[1].code, 80);
    }

    #[test]
    fn missing_values_leave_their_hour_out() {
        let json = r#"{
            "timezone": "GMT", "utc_offset_seconds": 0,
            "current": {"time": 0, "temperature_2m": 1.5, "weather_code": null},
            "hourly": {"time": [0, 3600], "temperature_2m": [null, 2.0], "weather_code": [1, 2], "is_day": [1, 1]},
            "daily": {"time": [0], "weather_code": [3], "temperature_2m_max": [4.0], "temperature_2m_min": [null]}
        }"#;
        let forecast = parse_forecast(json).unwrap();
        assert_eq!(forecast.current.code, -1);
        assert!(forecast.current.day);
        assert_eq!(forecast.hourly.len(), 1);
        assert_eq!(forecast.hourly[0].temperature, 2.0);
        assert!(forecast.daily.is_empty());
    }

    #[test]
    fn refusals_say_why() {
        let error = parse_forecast(
            r#"{"error": true, "reason": "Latitude must be in range of -90 to 90°."}"#,
        )
        .unwrap_err();
        assert_eq!(
            error,
            "Open-Meteo says: Latitude must be in range of -90 to 90°."
        );
        assert!(parse_forecast("{}").is_err());
    }

    #[test]
    fn places_are_found_by_name_and_country() {
        let lyon = find_place(GEOCODING_JSON, "Lyon").unwrap();
        assert_eq!(lyon.name, "Lyon");
        assert_eq!(lyon.country, "France");
        assert_eq!(lyon.region, "Rhône-Alpes");
        assert!((lyon.latitude - 45.749).abs() < 0.01);
        let american = find_place(GEOCODING_JSON, "Lyon, us").unwrap();
        assert_eq!(american.country, "United States");
        let missouri = find_place(GEOCODING_JSON, "Lyon, Missouri").unwrap();
        assert_eq!(missouri.region, "Missouri");
        let error = find_place(GEOCODING_JSON, "Lyon, Japan").unwrap_err();
        assert_eq!(error, "Open-Meteo knows no place called \"Lyon, Japan\"");
        // Open-Meteo leaves `results` out when nothing matches.
        assert!(find_place(r#"{"generationtime_ms": 0.7}"#, "Nowhere").is_err());
    }

    #[test]
    fn urls_ask_for_the_units() {
        let metric = forecast_url(45.75, 4.85, Units::Metric);
        assert!(
            metric.starts_with(
                "https://api.open-meteo.com/v1/forecast?latitude=45.75&longitude=4.85&"
            )
        );
        assert!(!metric.contains("fahrenheit"));
        assert!(metric.contains("forecast_days=7&forecast_hours=24"));
        let imperial = forecast_url(45.75, 4.85, Units::Imperial);
        assert!(imperial.ends_with("&temperature_unit=fahrenheit&wind_speed_unit=mph"));
        assert_eq!(
            (Units::Imperial.temperature(), Units::Imperial.speed()),
            ("°F", "mph")
        );
        assert_eq!(
            (Units::Metric.temperature(), Units::Metric.speed()),
            ("°C", "km/h")
        );
        assert_eq!(
            geocoding_url(looked_up("Saint-Étienne, France")),
            "https://geocoding-api.open-meteo.com/v1/search?count=10&format=json&name=Saint-%C3%89tienne"
        );
        assert_eq!(encode("New York"), "New%20York");
    }

    #[test]
    fn days_become_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
        assert_eq!(civil(20_735), (2026, 10, 9));
        assert_eq!(civil(11_016), (2000, 2, 29));
    }
}

//! Weather: the forecast from Open-Meteo, which needs no account or key,
//! for a place set by its name or by coordinates. Until one is set, the
//! module sends nothing at all, and its views say how to set one.
//!
//! A name is looked up once with Open-Meteo's geocoding, and where it is
//! stays in `$XDG_STATE_HOME/mochi/weather.json` with the last forecast,
//! so a restart shows the weather at once and asks again only when it's
//! due. The forecast is the weather now, the next 24 hours and the next 7
//! days, fetched again every `refresh_minutes`.
//!
//! The module's state has all of it, for any view to share: the control
//! center's cards and page and the desktop widget here, and a clock or a
//! lock screen elsewhere. Its settings apply without a restart.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.weather]
//! place = "Lyon, France"   # or latitude and longitude
//! units = "metric"         # or "imperial"
//! refresh_minutes = 30
//! stale_minutes = 60       # how old before the looks say so
//! ```

mod codes;
mod meteo;
mod tour;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, Assets, BoxFuture, ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError,
    ModuleEvent,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::meteo::{Current, Day, Fetch, Forecast, Hour, Place, Units};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long after start the first fetch waits, so it doesn't compete with
/// everything starting, in seconds.
const FIRST_FETCH: u64 = 3;
/// After a failed fetch the next waits this long, then twice as long each
/// time, up to `refresh_minutes`.
const RETRY: u64 = 60;
/// The longest the module sleeps. It goes by the wall clock, so a fetch
/// that came due while the computer slept runs soon after it wakes.
const TICK: Duration = Duration::from_secs(60);
/// A forecast older than this is too old to show, in seconds.
const STALE: u64 = 6 * 3600;
/// What the views and `refresh` say while nothing is set.
const UNSET: &str = "no place set: set `place`, or `latitude` and `longitude`, in [module.weather]";
/// Every setting applies at once, so none needs a restart.
const LIVE: [&str; 7] = [
    "prompt",
    "place",
    "latitude",
    "longitude",
    "units",
    "refresh_minutes",
    "stale_minutes",
];

#[derive(Debug, Default)]
pub struct Weather;

#[derive(Debug, Clone, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// A city or a town, like "Lyon", or "Lyon, France" for the one in that
    /// country or region. It's looked up once, with Open-Meteo.
    place: String,
    /// Where you are instead, with `longitude`, which skips the lookup:
    /// north is positive. `place` then only names it.
    #[schemars(range(min = -90, max = 90))]
    latitude: Option<f64>,
    /// East is positive.
    #[schemars(range(min = -180, max = 180))]
    longitude: Option<f64>,
    /// "metric" for °C and km/h, "imperial" for °F and mph.
    units: Units,
    /// How often to fetch the forecast again, in minutes.
    #[schemars(range(min = 10, max = 360))]
    refresh_minutes: u64,
    /// How old the forecast gets, in minutes, before the weather looks say
    /// how old it is, as when the computer was offline. Never less than
    /// twice `refresh_minutes`, so a forecast waiting for its next fetch
    /// doesn't count.
    #[schemars(range(min = 20, max = 360))]
    stale_minutes: u64,
    /// Until a place is set, the control center's weather cards say how to
    /// set one. Off, they stay hidden until then.
    prompt: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            place: String::new(),
            latitude: None,
            longitude: None,
            units: Units::Metric,
            refresh_minutes: 30,
            stale_minutes: 60,
            prompt: true,
        }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        // One of latitude and longitude alone is fine: the settings panel
        // sets them one at a time. They count once both are set.
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if let Some(latitude) = settings.latitude
            && !(-90.0..=90.0).contains(&latitude)
        {
            return Err(format!("latitude is {latitude}; it goes from -90 to 90"));
        }
        if let Some(longitude) = settings.longitude
            && !(-180.0..=180.0).contains(&longitude)
        {
            return Err(format!(
                "longitude is {longitude}; it goes from -180 to 180"
            ));
        }
        if !(10..=360).contains(&settings.refresh_minutes) {
            return Err(format!(
                "refresh_minutes is {}; it goes from 10 to 360",
                settings.refresh_minutes
            ));
        }
        if !(20..=360).contains(&settings.stale_minutes) {
            return Err(format!(
                "stale_minutes is {}; it goes from 20 to 360",
                settings.stale_minutes
            ));
        }
        Ok(settings)
    }

    fn location(&self) -> Location {
        let name = self.place.trim();
        match (self.latitude, self.longitude) {
            (Some(latitude), Some(longitude)) => Location::At(Place {
                name: if name.is_empty() {
                    format!("{latitude:.2}, {longitude:.2}")
                } else {
                    name.to_owned()
                },
                region: String::new(),
                country: String::new(),
                latitude,
                longitude,
            }),
            _ if !name.is_empty() => Location::Named(name.to_owned()),
            _ => Location::Unset,
        }
    }

    /// Seconds between two fetches.
    fn interval(&self) -> u64 {
        self.refresh_minutes.max(1) * 60
    }

    /// How old the forecast gets before the looks call it stale, in
    /// seconds.
    fn stale_after(&self) -> u64 {
        self.stale_minutes.max(self.refresh_minutes * 2) * 60
    }
}

/// Where the settings ask the weather for.
#[derive(Debug, Clone, PartialEq)]
enum Location {
    Unset,
    /// A name to look up.
    Named(String),
    At(Place),
}

impl Location {
    /// What a saved forecast is for, to know it's still the one wanted.
    fn key(&self) -> Option<String> {
        match self {
            Self::Unset => None,
            Self::Named(name) => Some(format!("name:{}", name.to_lowercase())),
            Self::At(place) => Some(format!(
                "at:{},{}:{}",
                place.latitude, place.longitude, place.name
            )),
        }
    }
}

/// `$XDG_STATE_HOME/mochi/weather.json`: the last name looked up and where
/// it is, so a name is looked up once, and the last forecast, so a restart
/// shows it at once.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    lookup: Option<Lookup>,
    last: Option<Last>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Lookup {
    /// The name as set, in lower case.
    query: String,
    place: Place,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Last {
    /// The [`Location::key`] it's for.
    key: String,
    units: Units,
    /// When it came, in seconds since the epoch.
    updated: u64,
    place: Place,
    forecast: Forecast,
}

impl Saved {
    fn path() -> Option<PathBuf> {
        Some(mochi_core::config::state_dir()?.join("weather.json"))
    }

    fn load(path: Option<&Path>) -> Self {
        path.and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| {
                serde_json::from_str(&text)
                    .inspect_err(|error| tracing::warn!(%error, "ignoring the saved weather"))
                    .ok()
            })
            .unwrap_or_default()
    }

    fn save(&self, path: Option<&Path>) {
        let Some(path) = path else {
            return;
        };
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(path, serde_json::to_vec(self).unwrap_or_default()));
        if let Err(error) = written {
            tracing::warn!(%error, "can't save the weather");
        }
    }
}

/// A fetch, to run off the module's loop.
#[derive(Debug, Clone)]
struct Job {
    generation: u64,
    location: Location,
    /// The last name looked up, which saves looking it up again.
    lookup: Option<Lookup>,
    units: Units,
}

/// What a fetch brought.
#[derive(Debug)]
struct Fetched {
    place: Place,
    forecast: Forecast,
    /// A new lookup, to keep.
    lookup: Option<Lookup>,
}

/// Why a fetch failed, and whether trying again later may help: a place
/// Open-Meteo doesn't know waits for the settings to change.
#[derive(Debug)]
struct Failure {
    message: String,
    again: bool,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self {
            message,
            again: true,
        }
    }
}

/// Looks the place up unless the job knows where it is, then fetches its
/// forecast. An unset location sends nothing.
async fn fetch(fetcher: &dyn Fetch, job: &Job) -> Result<Fetched, Failure> {
    let (place, lookup) = match &job.location {
        Location::Unset => {
            return Err(Failure {
                message: UNSET.to_owned(),
                again: false,
            });
        }
        Location::At(place) => (place.clone(), None),
        Location::Named(name) => {
            let query = name.to_lowercase();
            match &job.lookup {
                Some(known) if known.query == query => (known.place.clone(), None),
                _ => {
                    let json = fetcher
                        .get(meteo::geocoding_url(meteo::looked_up(name)))
                        .await?;
                    let place = meteo::find_place(&json, name).map_err(|message| Failure {
                        message,
                        again: false,
                    })?;
                    let lookup = Lookup {
                        query,
                        place: place.clone(),
                    };
                    (place, Some(lookup))
                }
            }
        }
    };
    let json = fetcher
        .get(meteo::forecast_url(
            place.latitude,
            place.longitude,
            job.units,
        ))
        .await?;
    let forecast = meteo::parse_forecast(&json)?;
    Ok(Fetched {
        place,
        forecast,
        lookup,
    })
}

impl Module for Weather {
    fn id(&self) -> &'static str {
        "weather"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("refresh", "Fetch the forecast now"),
            ActionSpec::new("status", "Print the weather now and today's"),
        ]
    }

    fn needs(&self, table: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        let settings = Settings::load(table).unwrap_or_default();
        if settings.location() == Location::Unset {
            return Vec::new();
        }
        vec![mochi_core::Need::new("curl", "Fetching the forecast")]
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            // After the default cards: until a place is set, it only says
            // how to set one.
            ContributionSpec::new("control-center", "card", "current", "Card", "Weather")
                .icon("partly_cloudy_day")
                .order(50)
                .options(json!({ "span": 2, "rows": 1 })),
            // The same in one column, for a home that has no room for the
            // wide one: it waits under More cards until it's put there.
            ContributionSpec::new("control-center", "card", "now", "Now", "Weather now")
                .icon("partly_cloudy_day")
                .order(51)
                .options(json!({ "span": 1, "rows": 1, "spare": true })),
            ContributionSpec::new("control-center", "page", "page", "Page", "Weather")
                .icon("partly_cloudy_day")
                .order(50),
            // Its looks: the first is the one placed widgets had before
            // there were others.
            ContributionSpec::new("widgets", "widget", "current", "Widget", "Weather")
                .icon("partly_cloudy_day")
                .options(json!({
                    "size": [16, 6],
                    "min": [10, 5],
                    "max": [40, 20],
                    "category": "Weather",
                    "variants": [
                        {
                            "id": "current",
                            "title": "Now",
                            "description": "The sky and the temperature now, with the place",
                        },
                        {
                            "id": "icon",
                            "title": "Icon",
                            "description": "The sky's icon in a cookie, the temperature beside",
                            "size": [16, 8],
                        },
                        {
                            "id": "forecast",
                            "title": "Forecast",
                            "description": "The next days, each with its low and high",
                            "view": "Forecast",
                            "size": [20, 14],
                            "min": [18, 8],
                            "max": [40, 30],
                        },
                        {
                            "id": "hours",
                            "title": "Hours",
                            "description": "The next 12 hours as a curve, with their skies",
                            "view": "Hours",
                            "size": [24, 10],
                            "min": [16, 8],
                            "max": [60, 24],
                        },
                    ],
                })),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        Settings::load(table).map(drop)
    }

    fn live_settings(&self) -> &'static [&'static str] {
        &LIVE
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused the wrong ones.
            let settings: Settings = ctx.settings()?;
            let fetcher: Arc<dyn Fetch> = Arc::new(meteo::Curl);
            let (done, mut finished) = tokio::sync::mpsc::unbounded_channel();
            let mut weather = State::new(settings, Saved::path(), now());
            let mut published = Value::Null;
            loop {
                if let Some(job) = weather.begin(now()) {
                    let fetcher = fetcher.clone();
                    let done = done.clone();
                    tokio::spawn(async move {
                        let result = fetch(&*fetcher, &job).await;
                        let _ = done.send((job.generation, result));
                    });
                }
                // The hours that went by leave the state too, so it's
                // compared rather than sent on every tick.
                let state = weather.state(now());
                if state != published {
                    ctx.publish_state(state.clone());
                    published = state;
                }
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => weather.command(command, now()),
                        Some(ModuleEvent::Reconfigured(table)) => match Settings::load(&table) {
                            Ok(settings) => weather.reconfigure(settings, now()),
                            Err(error) => tracing::warn!(%error, "the weather's new settings"),
                        },
                        Some(_) => {}
                    },
                    Some((generation, result)) = finished.recv() => {
                        weather.finish(generation, result, now());
                    }
                    () = tokio::time::sleep(weather.wait(now())) => {}
                }
            }
        })
    }
}

/// The wall clock, in seconds since the epoch.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

struct State {
    settings: Settings,
    /// Where [`Saved`] goes; none in tests that don't save.
    path: Option<PathBuf>,
    saved: Saved,
    /// Counts the changes of place and units: a fetch from before one is
    /// dropped when it comes back.
    generation: u64,
    /// The generation of the fetch that runs.
    running: Option<u64>,
    /// When the next fetch is due, in seconds since the epoch; none while
    /// nothing is set, or after a failure that waits for new settings.
    due: Option<u64>,
    /// How long the next try after a failure waits.
    retry: u64,
    error: Option<String>,
}

impl State {
    fn new(settings: Settings, path: Option<PathBuf>, now: u64) -> Self {
        let saved = Saved::load(path.as_deref());
        let mut state = Self {
            settings,
            path,
            saved,
            generation: 0,
            running: None,
            due: None,
            retry: RETRY,
            error: None,
        };
        state.due = state.first_due(now + FIRST_FETCH);
        state
    }

    /// When the first fetch for the settings is due: when the saved
    /// forecast for them gets old, or `soonest`.
    fn first_due(&self, soonest: u64) -> Option<u64> {
        if self.settings.location() == Location::Unset {
            return None;
        }
        Some(
            self.last()
                .map_or(soonest, |last| last.updated + self.settings.interval())
                .max(soonest),
        )
    }

    /// The last forecast, when it's for the place and the units set.
    fn last(&self) -> Option<&Last> {
        let key = self.settings.location().key()?;
        self.saved
            .last
            .as_ref()
            .filter(|last| last.key == key && last.units == self.settings.units)
    }

    /// The fetch to start, when one is due and none runs.
    fn begin(&mut self, now: u64) -> Option<Job> {
        if self.running.is_some() || self.due.is_none_or(|due| due > now) {
            return None;
        }
        self.due = None;
        let location = self.settings.location();
        if location == Location::Unset {
            return None;
        }
        self.running = Some(self.generation);
        Some(Job {
            generation: self.generation,
            location,
            lookup: self.saved.lookup.clone(),
            units: self.settings.units,
        })
    }

    fn finish(&mut self, generation: u64, result: Result<Fetched, Failure>, now: u64) {
        if self.running != Some(generation) {
            return;
        }
        self.running = None;
        match result {
            Ok(fetched) => {
                if let Some(lookup) = fetched.lookup {
                    self.saved.lookup = Some(lookup);
                }
                if let Some(key) = self.settings.location().key() {
                    self.saved.last = Some(Last {
                        key,
                        units: self.settings.units,
                        updated: now,
                        place: fetched.place,
                        forecast: fetched.forecast,
                    });
                }
                self.saved.save(self.path.as_deref());
                self.error = None;
                self.retry = RETRY;
                self.due = Some(now + self.settings.interval());
            }
            Err(failure) => {
                tracing::info!(error = %failure.message, "no weather");
                self.error = Some(failure.message);
                self.due = failure.again.then_some(now + self.retry);
                self.retry = (self.retry * 2).min(self.settings.interval());
            }
        }
    }

    fn reconfigure(&mut self, settings: Settings, now: u64) {
        let moved = settings.location() != self.settings.location()
            || settings.units != self.settings.units;
        self.settings = settings;
        if moved {
            // What runs is for the old place: its answer is dropped.
            self.generation += 1;
            self.running = None;
            self.error = None;
            self.retry = RETRY;
            self.due = self.first_due(now);
        } else if self.running.is_none() && self.error.is_none() {
            // A new interval counts from the last fetch.
            self.due = self.first_due(now);
        }
    }

    fn refresh(&mut self, now: u64) -> Result<(), String> {
        if self.settings.location() == Location::Unset {
            return Err(UNSET.to_owned());
        }
        if self.running.is_none() {
            self.due = Some(now);
        }
        Ok(())
    }

    fn command(&mut self, command: ModuleCommand, now: u64) {
        match command.action.as_str() {
            "refresh" => command.reply(self.refresh(now)),
            "status" => command.answer(Ok(self.status(now))),
            other => {
                let error = format!("weather has no action {other}");
                command.reply(Err(error));
            }
        }
    }

    /// How long the loop may sleep.
    fn wait(&self, now: u64) -> Duration {
        match self.due {
            Some(due) => Duration::from_secs(due.saturating_sub(now)).min(TICK),
            None => TICK,
        }
    }

    /// The forecast to show: the last one, unless it's too old.
    fn shown(&self, now: u64) -> Option<&Last> {
        self.last()
            .filter(|last| now.saturating_sub(last.updated) < STALE)
    }

    /// The place to name: the forecast's, or where the settings say before
    /// one came.
    fn place(&self, now: u64) -> Option<Place> {
        if let Some(last) = self.shown(now) {
            return Some(last.place.clone());
        }
        match self.settings.location() {
            Location::Unset => None,
            Location::At(place) => Some(place),
            Location::Named(name) => self
                .saved
                .lookup
                .as_ref()
                .filter(|known| known.query == name.to_lowercase())
                .map(|known| known.place.clone()),
        }
    }

    /// The module's state, which every view shares. See the weather page
    /// in the book for each field.
    fn state(&self, now: u64) -> Value {
        let units = self.settings.units;
        let shown = self.shown(now);
        let forecast = shown.map(|last| &last.forecast);
        let offset = forecast.map_or(0, |forecast| forecast.utc_offset);
        let now = now as i64;
        json!({
            "configured": self.settings.location() != Location::Unset,
            "loading": self.running.is_some(),
            "error": self.error,
            // None while a fetch runs, and after a failure that waits for
            // new settings.
            "next": self.due,
            "units": units.name(),
            "unit": {
                "temperature": units.temperature(),
                "speed": units.speed(),
            },
            "place": self.place(now as u64),
            "updated": shown.map(|last| last.updated),
            // How old `updated` gets before the looks say so, in seconds.
            "stale_after": self.settings.stale_after(),
            // Whether the cards show "Set a place" before a place is set.
            "prompt": self.settings.prompt,
            "timezone": forecast.map(|forecast| &forecast.timezone),
            "utc_offset": forecast.map(|forecast| forecast.utc_offset),
            "current": forecast.map(|forecast| current(&forecast.current)),
            // From the hour that runs now.
            "hourly": forecast.map_or(Vec::new(), |forecast| {
                forecast
                    .hourly
                    .iter()
                    .filter(|hour| hour.time + 3600 > now)
                    .map(|hour| hourly(hour, offset))
                    .collect()
            }),
            // From today, at the place.
            "daily": forecast.map_or(Vec::new(), |forecast| {
                forecast
                    .daily
                    .iter()
                    .filter(|day| day.time + 86_400 > now)
                    .map(|day| daily(day, offset))
                    .collect()
            }),
        })
    }

    /// What `status` prints.
    fn status(&self, now: u64) -> String {
        let Some(last) = self.shown(now) else {
            return match (&self.settings.location(), &self.error) {
                (Location::Unset, _) => UNSET.to_owned(),
                (_, _) if self.running.is_some() => "fetching the forecast".to_owned(),
                (_, Some(error)) => format!("no forecast: {error}"),
                (_, None) => "no forecast yet".to_owned(),
            };
        };
        let unit = self.settings.units.temperature();
        let degrees = |value: f64| format!("{}{unit}", value.round());
        let now_there = &last.forecast.current;
        let sky = codes::sky(now_there.code, now_there.day);
        let mut text = format!(
            "{}: {}, {}",
            last.place.name,
            degrees(now_there.temperature),
            sky.text.to_lowercase()
        );
        if let Some(feels_like) = now_there.feels_like {
            text.push_str(&format!(", feels like {}", degrees(feels_like)));
        }
        if let Some(today) = last.forecast.daily.first() {
            text.push_str(&format!(
                "; today {} to {}",
                degrees(today.min),
                degrees(today.max)
            ));
        }
        let minutes = now.saturating_sub(last.updated) / 60;
        text.push_str(&match minutes {
            0 => ". Updated just now".to_owned(),
            1 => ". Updated a minute ago".to_owned(),
            minutes => format!(". Updated {minutes} minutes ago"),
        });
        if let Some(error) = &self.error {
            text.push_str(&format!("; the last try failed: {error}"));
        }
        text
    }
}

/// The weather now, with its words and icon.
fn current(now: &Current) -> Value {
    let sky = codes::sky(now.code, now.day);
    json!({
        "time": now.time,
        "temperature": now.temperature,
        "feels_like": now.feels_like,
        "humidity": now.humidity,
        "wind_speed": now.wind_speed,
        "wind_direction": now.wind_direction,
        "uv_index": now.uv_index,
        "code": now.code,
        "day": now.day,
        "text": sky.text,
        "icon": sky.icon,
        "kind": sky.kind,
    })
}

/// An hour, with its hour of the day at the place.
fn hourly(hour: &Hour, offset: i64) -> Value {
    let sky = codes::sky(hour.code, hour.day);
    json!({
        "time": hour.time,
        "hour": (hour.time + offset).rem_euclid(86_400) / 3600,
        "temperature": hour.temperature,
        "precipitation": hour.precipitation,
        "code": hour.code,
        "day": hour.day,
        "text": sky.text,
        "icon": sky.icon,
        "kind": sky.kind,
    })
}

/// A day, with its date and day of the week at the place.
fn daily(day: &Day, offset: i64) -> Value {
    let sky = codes::sky(day.code, true);
    let days = (day.time + offset).div_euclid(86_400);
    let (year, month, date) = meteo::civil(days);
    json!({
        "time": day.time,
        "date": format!("{year:04}-{month:02}-{date:02}"),
        // 0 is Sunday, like JavaScript's getDay().
        "weekday": (days + 4).rem_euclid(7),
        "min": day.min,
        "max": day.max,
        "precipitation": day.precipitation,
        "uv_index": day.uv_index,
        "sunrise": day.sunrise,
        "sunset": day.sunset,
        "code": day.code,
        "text": sky.text,
        "icon": sky.icon,
        "kind": sky.kind,
    })
}

#[cfg(test)]
mod fixtures {
    /// Real answers from Open-Meteo, for Lyon on 2026-10-09 at 20:45.
    pub const FORECAST: &str = include_str!("../fixtures/forecast.json");
    pub const GEOCODING: &str = include_str!("../fixtures/geocoding.json");
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// When the fixture's forecast came, 20:45 in Lyon.
    const THEN: u64 = 1_791_571_500;

    /// Answers with the fixtures, and keeps every URL asked.
    #[derive(Debug, Default)]
    struct Fixtures {
        asked: Mutex<Vec<String>>,
        down: bool,
    }

    impl Fixtures {
        fn asked(&self) -> Vec<String> {
            self.asked.lock().unwrap().clone()
        }
    }

    impl Fetch for Fixtures {
        fn get(&self, url: String) -> BoxFuture<'static, Result<String, String>> {
            self.asked.lock().unwrap().push(url.clone());
            let answer = if self.down {
                Err("can't reach Open-Meteo: Could not resolve host")
            } else if url.starts_with(meteo::GEOCODING) {
                Ok(fixtures::GEOCODING)
            } else if url.starts_with(meteo::FORECAST) {
                Ok(fixtures::FORECAST)
            } else {
                Err("no such fixture")
            };
            Box::pin(async move { answer.map(str::to_owned).map_err(str::to_owned) })
        }
    }

    #[test]
    fn every_view_offered_ships() {
        for offer in Weather.contributions() {
            let mut views = vec![offer.view.clone()];
            for variant in offer.options["variants"].as_array().into_iter().flatten() {
                views.extend(variant["view"].as_str().map(str::to_owned));
            }
            for view in views {
                assert!(
                    QML.get_file(format!("{view}.qml")).is_some(),
                    "{} offers {view}, which isn't in qml/",
                    offer.id
                );
            }
        }
    }

    fn settings(text: &str) -> Settings {
        Settings::load(&mochi_core::toml::from_str(text).unwrap()).unwrap()
    }

    fn temp(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mochi-weather-{name}-{}/weather.json",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        path
    }

    /// Runs the fetch that's due at `now`, if one is, and says whether one
    /// was.
    async fn step(state: &mut State, fetcher: &Fixtures, now: u64) -> bool {
        let Some(job) = state.begin(now) else {
            return false;
        };
        let result = fetch(fetcher, &job).await;
        state.finish(job.generation, result, now);
        true
    }

    #[test]
    fn the_example_matches_the_settings() {
        mochi_core::examples::check_module::<Settings>("weather", include_str!("../settings.toml"));
    }

    #[test]
    fn every_setting_applies_live() {
        let schema = mochi_core::options::schema_of::<Settings>();
        let mut keys: Vec<&str> = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let mut live = LIVE.to_vec();
        live.sort_unstable();
        assert_eq!(keys, live);
    }

    #[test]
    fn the_forecast_goes_stale_after_two_fetches_at_least() {
        // An hour by default, as before the setting.
        assert_eq!(settings("").stale_after(), 3600);
        assert_eq!(settings("stale_minutes = 180").stale_after(), 3 * 3600);
        // A forecast fetched every two hours isn't stale after one.
        assert_eq!(settings("refresh_minutes = 120").stale_after(), 4 * 3600);
        assert!(
            Settings::load(&mochi_core::toml::from_str("stale_minutes = 5").unwrap())
                .unwrap_err()
                .contains("20 to 360")
        );
    }

    #[test]
    fn settings_are_checked() {
        let load = |text: &str| Settings::load(&mochi_core::toml::from_str(text).unwrap());
        // Half the coordinates wait for the other half.
        assert_eq!(settings("latitude = 45.7").location(), Location::Unset);
        assert_eq!(
            settings("latitude = 45.7\nplace = \"Lyon\"").location(),
            Location::Named("Lyon".into())
        );
        assert!(
            load("latitude = 95.0\nlongitude = 4.8")
                .unwrap_err()
                .contains("-90 to 90")
        );
        assert!(load("latitude = 45.0\nlongitude = 200.0").is_err());
        assert!(
            load("refresh_minutes = 1")
                .unwrap_err()
                .contains("10 to 360")
        );
        assert!(load("units = \"kelvin\"").is_err());
        assert!(load("city = \"Lyon\"").is_err());
        assert_eq!(settings("").location(), Location::Unset);
        assert_eq!(settings("place = \"  \"").location(), Location::Unset);
        assert_eq!(
            settings("place = \" Lyon \"").location(),
            Location::Named("Lyon".into())
        );
        let Location::At(place) = settings("latitude = 45.75\nlongitude = 4.85").location() else {
            panic!("coordinates are a place");
        };
        assert_eq!(place.name, "45.75, 4.85");
        let Location::At(home) =
            settings("place = \"Home\"\nlatitude = 45.75\nlongitude = 4.85").location()
        else {
            panic!("coordinates win over the name");
        };
        assert_eq!(home.name, "Home");
    }

    #[tokio::test]
    async fn no_place_sends_no_request() {
        let fetcher = Fixtures::default();
        let mut state = State::new(Settings::default(), None, THEN);
        for minutes in [0, 1, 60, 24 * 60] {
            assert!(!step(&mut state, &fetcher, THEN + minutes * 60).await);
        }
        assert_eq!(state.refresh(THEN).unwrap_err(), UNSET);
        assert!(!step(&mut state, &fetcher, THEN + 60).await);
        assert!(fetcher.asked().is_empty());
        let published = state.state(THEN);
        assert_eq!(published["configured"], false);
        assert_eq!(published["loading"], false);
        assert!(published["current"].is_null());
        assert!(published["next"].is_null());
        assert!(published["place"].is_null());
        assert_eq!(state.status(THEN), UNSET);
        assert!(Weather.needs(&mochi_core::toml::Table::new()).is_empty());
        // Taking the place away stops the fetches.
        let mut state = State::new(settings("place = \"Lyon\""), None, THEN);
        state.reconfigure(Settings::default(), THEN);
        assert!(!step(&mut state, &fetcher, THEN + 3600).await);
        assert!(fetcher.asked().is_empty());
    }

    #[tokio::test]
    async fn a_name_is_looked_up_once() {
        let path = temp("lookup");
        let fetcher = Fixtures::default();
        let mut state = State::new(
            settings("place = \"Lyon, France\""),
            Some(path.clone()),
            THEN,
        );
        // The first fetch waits a moment after the start.
        assert!(!step(&mut state, &fetcher, THEN).await);
        assert!(step(&mut state, &fetcher, THEN + FIRST_FETCH).await);
        let asked = fetcher.asked();
        assert_eq!(asked.len(), 2);
        assert!(asked[0].starts_with(meteo::GEOCODING), "{asked:?}");
        assert!(asked[0].ends_with("&name=Lyon"), "{asked:?}");
        assert!(
            asked[1].contains("latitude=45.74906&longitude=4.84789"),
            "{asked:?}"
        );

        // Not due again before refresh_minutes, then only the forecast.
        assert!(!step(&mut state, &fetcher, THEN + 29 * 60).await);
        assert!(step(&mut state, &fetcher, THEN + 30 * 60 + FIRST_FETCH).await);
        assert_eq!(fetcher.asked().len(), 3);
        assert!(fetcher.asked()[2].starts_with(meteo::FORECAST));

        // After a restart, the saved forecast shows at once, and the next
        // fetch needs no lookup either, also with the name typed otherwise.
        let fetcher = Fixtures::default();
        let later = THEN + 30 * 60 + FIRST_FETCH;
        let mut state = State::new(
            settings("place = \"lyon, france\""),
            Some(path.clone()),
            later + 60,
        );
        assert_eq!(state.state(later + 60)["place"]["name"], "Lyon");
        assert!(!step(&mut state, &fetcher, later + 120).await);
        state.refresh(later + 120).unwrap();
        assert!(step(&mut state, &fetcher, later + 120).await);
        assert_eq!(fetcher.asked().len(), 1);
        assert!(fetcher.asked()[0].starts_with(meteo::FORECAST));

        // Another place is looked up, at once.
        state.reconfigure(settings("place = \"Lyon, Missouri\""), later + 180);
        assert!(state.state(later + 180)["place"].is_null());
        assert!(step(&mut state, &fetcher, later + 180).await);
        assert!(fetcher.asked()[1].starts_with(meteo::GEOCODING));
        assert_eq!(state.state(later + 180)["place"]["region"], "Missouri");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn coordinates_need_no_lookup() {
        let fetcher = Fixtures::default();
        let mut state = State::new(
            settings("latitude = 45.75\nlongitude = 4.85\nunits = \"imperial\""),
            None,
            THEN,
        );
        assert_eq!(state.state(THEN)["place"]["name"], "45.75, 4.85");
        assert!(step(&mut state, &fetcher, THEN + FIRST_FETCH).await);
        let asked = fetcher.asked();
        assert_eq!(asked.len(), 1);
        assert!(asked[0].contains("latitude=45.75&longitude=4.85"));
        assert!(asked[0].contains("temperature_unit=fahrenheit"));
        let published = state.state(THEN);
        assert_eq!(published["units"], "imperial");
        assert_eq!(published["unit"]["temperature"], "°F");
        assert_eq!(published["unit"]["speed"], "mph");
    }

    #[tokio::test]
    async fn the_state_has_the_whole_forecast() {
        let fetcher = Fixtures::default();
        let mut state = State::new(settings("place = \"Lyon\""), None, THEN);
        assert!(step(&mut state, &fetcher, THEN + FIRST_FETCH).await);
        let published = state.state(THEN);
        assert_eq!(published["configured"], true);
        assert_eq!(published["loading"], false);
        assert!(published["error"].is_null());
        assert_eq!(published["updated"], THEN + FIRST_FETCH);
        assert_eq!(published["next"], THEN + FIRST_FETCH + 30 * 60);
        assert_eq!(published["timezone"], "Europe/Paris");
        assert_eq!(published["utc_offset"], 7200);
        assert_eq!(published["place"]["country"], "France");
        assert_eq!(published["unit"]["temperature"], "°C");

        let current = &published["current"];
        assert_eq!(current["temperature"], 14.8);
        assert_eq!(current["feels_like"], 12.9);
        assert_eq!(current["humidity"], 52.0);
        assert_eq!(current["wind_speed"], 4.7);
        assert_eq!(current["uv_index"], 0.0);
        assert_eq!(current["text"], "Overcast");
        assert_eq!(current["icon"], "cloud");
        assert_eq!(current["day"], false);

        let hours = published["hourly"].as_array().unwrap();
        assert_eq!(hours.len(), 24);
        // 20:00 in Lyon, the hour that runs.
        assert_eq!(hours[0]["hour"], 20);
        assert_eq!(hours[0]["temperature"], 15.1);
        assert_eq!(hours[0]["icon"], "cloud");
        assert_eq!(hours[4]["hour"], 0);

        let days = published["daily"].as_array().unwrap();
        assert_eq!(days.len(), 7);
        assert_eq!(days[0]["date"], "2026-10-09");
        assert_eq!(days[0]["weekday"], 5);
        assert_eq!(days[0]["min"], 9.8);
        assert_eq!(days[0]["max"], 16.3);
        assert_eq!(days[1]["text"], "Light showers");
        assert_eq!(days[1]["precipitation"], 75.0);
        assert_eq!(days[6]["date"], "2026-10-15");

        assert_eq!(
            state.status(THEN + 10 * 60),
            "Lyon: 15°C, overcast, feels like 13°C; today 10°C to 16°C. Updated 9 minutes ago"
        );

        // Hours and days that went by leave; a forecast too old goes.
        let tomorrow = THEN + 4 * 3600;
        let published = state.state(tomorrow);
        assert_eq!(published["hourly"][0]["hour"], 0);
        assert_eq!(published["daily"][0]["date"], "2026-10-10");
        assert!(state.state(THEN + STALE + 60)["current"].is_null());
    }

    #[tokio::test]
    async fn failures_try_again_later() {
        let down = Fixtures {
            down: true,
            ..Fixtures::default()
        };
        let mut state = State::new(settings("place = \"Lyon\""), None, THEN);
        assert!(step(&mut state, &down, THEN + FIRST_FETCH).await);
        let published = state.state(THEN + FIRST_FETCH);
        assert!(
            published["error"]
                .as_str()
                .unwrap()
                .contains("Could not resolve")
        );
        assert!(published["current"].is_null());
        assert_eq!(published["next"], THEN + FIRST_FETCH + RETRY);
        // A minute later, then two.
        let first = THEN + FIRST_FETCH;
        assert!(!step(&mut state, &down, first + RETRY - 1).await);
        assert!(step(&mut state, &down, first + RETRY).await);
        assert!(!step(&mut state, &down, first + RETRY * 3 - 1).await);
        assert!(step(&mut state, &down, first + RETRY * 3).await);
        // Back online: the error goes.
        let fetcher = Fixtures::default();
        assert!(step(&mut state, &fetcher, first + RETRY * 7).await);
        assert!(state.state(first + RETRY * 7)["error"].is_null());

        // A place Open-Meteo doesn't know waits for new settings.
        let mut state = State::new(settings("place = \"Lyon, Japan\""), None, THEN);
        assert!(step(&mut state, &fetcher, THEN + FIRST_FETCH).await);
        assert!(state.status(THEN + FIRST_FETCH).contains("no place called"));
        assert!(state.state(THEN + FIRST_FETCH)["next"].is_null());
        assert!(!step(&mut state, &fetcher, THEN + 24 * 3600).await);
    }

    #[tokio::test]
    async fn an_answer_for_the_old_place_is_dropped() {
        let fetcher = Fixtures::default();
        let mut state = State::new(settings("place = \"Lyon\""), None, THEN);
        let job = state.begin(THEN + FIRST_FETCH).unwrap();
        assert_eq!(state.state(THEN)["loading"], true);
        assert_eq!(state.status(THEN), "fetching the forecast");
        state.reconfigure(settings("place = \"Lyon\"\nunits = \"imperial\""), THEN + 5);
        let result = fetch(&fetcher, &job).await;
        state.finish(job.generation, result, THEN + 5);
        assert!(state.state(THEN + 5)["current"].is_null());
        // The new one starts at once.
        let job = state.begin(THEN + 5).unwrap();
        assert_eq!(job.units, Units::Imperial);
    }
}

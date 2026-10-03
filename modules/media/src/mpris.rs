//! MPRIS media players on the session bus.
//!
//! One task watches which players come and go, and one task per player reads
//! its properties again every time they change. Each read becomes an
//! [`Update`] with the whole player, so nothing downstream has to merge
//! partial changes.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::names::{InterfaceName, OwnedBusName};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{Connection, proxy};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT: InterfaceName<'static> =
    InterfaceName::from_static_str_unchecked("org.mpris.MediaPlayer2");
const PLAYER: InterfaceName<'static> =
    InterfaceName::from_static_str_unchecked("org.mpris.MediaPlayer2.Player");
/// The track id players use when they have none.
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";
/// playerctld mirrors whichever player it considers active, so following it
/// would show every player twice.
const PLAYERCTLD: &str = "org.mpris.MediaPlayer2.playerctld";

#[proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2",
    gen_blocking = false
)]
trait Player {
    fn play_pause(&self) -> zbus::Result<()>;
    fn play(&self) -> zbus::Result<()>;
    fn pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    /// Moves by `offset` microseconds.
    fn seek(&self, offset: i64) -> zbus::Result<()>;
    /// Moves to `position` microseconds into `track_id`.
    fn set_position(&self, track_id: &ObjectPath<'_>, position: i64) -> zbus::Result<()>;

    #[zbus(signal)]
    fn seeked(&self, position: i64) -> zbus::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    /// The player's id for the track, needed to seek to a position.
    pub id: Option<String>,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    /// `file://` or `http(s)://`.
    pub art: Option<String>,
    pub length: Option<Duration>,
}

/// Everything the module shows about one player, as read at `read_at`.
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    /// The name the player gives itself, like "Spotify".
    pub identity: String,
    pub status: Status,
    pub track: Track,
    /// Where playback was at `read_at`.
    pub position: Option<Duration>,
    pub read_at: SystemTime,
    /// Playback speed, 1.0 normally.
    pub rate: f64,
    pub can_previous: bool,
    pub can_next: bool,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_seek: bool,
}

impl Player {
    /// The position now, assuming playback went on at `rate` since it was
    /// read.
    pub fn position_at(&self, now: SystemTime) -> Option<Duration> {
        let position = self.position?;
        if self.status != Status::Playing {
            return Some(position);
        }
        let elapsed = now.duration_since(self.read_at).unwrap_or_default();
        let moved = elapsed.mul_f64(self.rate.max(0.0));
        let position = position + moved;
        Some(match self.track.length {
            Some(length) => position.min(length),
            None => position,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    /// A player appeared or changed. `bus` is its well-known bus name.
    Changed { bus: String, player: Box<Player> },
    /// A player went away.
    Gone { bus: String },
}

/// Sends an [`Update`] for every player now, then for every change, until
/// nobody listens any more.
pub async fn watch(connection: Connection, updates: UnboundedSender<Update>) {
    if let Err(error) = discover(&connection, &updates).await {
        tracing::warn!(%error, "stopped watching media players");
    }
}

async fn discover(connection: &Connection, updates: &UnboundedSender<Update>) -> zbus::Result<()> {
    let bus = DBusProxy::new(connection).await?;
    // Subscribe before listing, so no player slips in between.
    let mut owners = bus.receive_name_owner_changed().await?;
    let mut followers = Followers::default();
    for name in bus.list_names().await? {
        if is_player(&name) {
            followers.add(connection, &bus, name, updates).await;
        }
    }

    while let Some(signal) = owners.next().await {
        let args = signal.args()?;
        if !is_player(args.name()) {
            continue;
        }
        let name = OwnedBusName::from(args.name().to_owned());
        if let Some(follower) = followers.0.remove(name.as_str()) {
            // Wait for it to stop, so none of its updates come after `Gone`.
            follower.task.abort();
            let _ = follower.task.await;
            let bus = name.to_string();
            if updates.send(Update::Gone { bus }).is_err() {
                return Ok(());
            }
        }
        if args.new_owner().is_some() {
            followers.add(connection, &bus, name, updates).await;
        }
    }
    Ok(())
}

/// One task per player, by bus name.
#[derive(Debug, Default)]
struct Followers(HashMap<String, Follower>);

#[derive(Debug)]
struct Follower {
    /// The player's process, or its bus name when the bus won't say.
    process: String,
    task: JoinHandle<()>,
}

impl Followers {
    /// Follows `name`, unless its process already has a player followed:
    /// VLC, for one, owns two names for the same player.
    async fn add(
        &mut self,
        connection: &Connection,
        bus: &DBusProxy<'_>,
        name: OwnedBusName,
        updates: &UnboundedSender<Update>,
    ) {
        let process = match bus
            .get_connection_unix_process_id(name.inner().clone())
            .await
        {
            Ok(pid) => pid.to_string(),
            Err(_) => name.to_string(),
        };
        if self.0.values().any(|follower| follower.process == process) {
            return;
        }
        tracing::debug!(%name, %process, "following a player");
        let task = spawn_follower(connection, name.clone(), updates);
        self.0.insert(name.to_string(), Follower { process, task });
    }
}

fn is_player(name: &str) -> bool {
    name.starts_with(PREFIX) && name != PLAYERCTLD
}

fn spawn_follower(
    connection: &Connection,
    bus: OwnedBusName,
    updates: &UnboundedSender<Update>,
) -> JoinHandle<()> {
    let connection = connection.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        if let Err(error) = follow(&connection, &bus, &updates).await {
            // Usually the player quit while we read it; `Gone` follows.
            tracing::debug!(%error, %bus, "stopped following a player");
        }
    })
}

/// Reads the player now and after every change of its playback properties.
async fn follow(
    connection: &Connection,
    bus: &OwnedBusName,
    updates: &UnboundedSender<Update>,
) -> zbus::Result<()> {
    let properties = PropertiesProxy::builder(connection)
        .destination(bus.clone())?
        .path(PATH)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let player = player_proxy(connection, bus).await?;
    let mut changes = properties.receive_properties_changed().await?;
    let mut seeks = player.receive_seeked().await?;

    let identity = match properties.get(ROOT, "Identity").await {
        Ok(value) => string(&value).unwrap_or_default(),
        Err(_) => String::new(),
    };
    let identity = if identity.is_empty() {
        short_name(bus).to_owned()
    } else {
        identity
    };

    loop {
        let values = properties.get_all(PLAYER).await?;
        let player = Box::new(parse(identity.clone(), &values, SystemTime::now()));
        let update = Update::Changed {
            bus: bus.to_string(),
            player,
        };
        if updates.send(update).is_err() {
            return Ok(());
        }

        // Wait for a change worth reading again.
        loop {
            tokio::select! {
                change = changes.next() => {
                    let Some(change) = change else { return Ok(()) };
                    if change.args()?.interface_name == PLAYER {
                        break;
                    }
                }
                seek = seeks.next() => {
                    if seek.is_none() {
                        return Ok(());
                    }
                    break;
                }
            }
        }
    }
}

async fn player_proxy(connection: &Connection, bus: &str) -> zbus::Result<PlayerProxy<'static>> {
    PlayerProxy::builder(connection)
        .destination(bus.to_owned())?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

/// `org.mpris.MediaPlayer2.firefox.instance_1_35` is `firefox`.
pub fn short_name(bus: &str) -> &str {
    let rest = bus.strip_prefix(PREFIX).unwrap_or(bus);
    rest.split('.').next().unwrap_or(rest)
}

/// What the module can ask a player to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Control {
    PlayPause,
    Play,
    Pause,
    Next,
    Previous,
    /// Seek to this position in the current track.
    SeekTo(Duration),
}

/// Sends `control` to the player on `bus`. `player` is its last known state,
/// which seeking needs.
pub async fn send(
    connection: &Connection,
    bus: &str,
    player: &Player,
    control: Control,
) -> zbus::Result<()> {
    let proxy = player_proxy(connection, bus).await?;
    match control {
        Control::PlayPause => proxy.play_pause().await,
        Control::Play => proxy.play().await,
        Control::Pause => proxy.pause().await,
        Control::Next => proxy.next().await,
        Control::Previous => proxy.previous().await,
        Control::SeekTo(target) => {
            let target = micros(target);
            // Positions belong to a track id. Without one, a relative seek
            // from where playback is now gets there too.
            match player.track.id.as_deref().map(ObjectPath::try_from) {
                Some(Ok(track)) => proxy.set_position(&track, target).await,
                _ => {
                    let now = player.position_at(SystemTime::now()).unwrap_or_default();
                    proxy.seek(target - micros(now)).await
                }
            }
        }
    }
}

fn micros(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

/// Builds a player from the properties of its `Player` interface. Missing or
/// malformed properties take neutral values: players differ a lot here.
fn parse(identity: String, values: &HashMap<String, OwnedValue>, read_at: SystemTime) -> Player {
    let get = |name: &str| values.get(name).map(|value| &**value);
    let flag = |name: &str| get(name).and_then(boolean).unwrap_or(false);

    let status = match get("PlaybackStatus").and_then(string).as_deref() {
        Some("Playing") => Status::Playing,
        Some("Paused") => Status::Paused,
        _ => Status::Stopped,
    };
    Player {
        identity,
        status,
        track: get("Metadata").map(track).unwrap_or_default(),
        position: get("Position").and_then(integer).map(duration_from_micros),
        read_at,
        rate: get("Rate").and_then(float).unwrap_or(1.0),
        can_previous: flag("CanGoPrevious"),
        can_next: flag("CanGoNext"),
        can_play: flag("CanPlay"),
        can_pause: flag("CanPause"),
        can_seek: flag("CanSeek"),
    }
}

fn track(metadata: &Value<'_>) -> Track {
    let Value::Dict(dict) = inner(metadata) else {
        return Track::default();
    };
    let mut track = Track::default();
    for (key, value) in dict.iter() {
        let Some(key) = string(key) else { continue };
        match key.as_str() {
            "mpris:trackid" => {
                track.id = string(value).filter(|id| !id.is_empty() && id != NO_TRACK);
            }
            "xesam:title" => track.title = string(value).unwrap_or_default(),
            "xesam:artist" => track.artists = strings(value),
            "xesam:album" => track.album = string(value).unwrap_or_default(),
            "mpris:artUrl" => track.art = string(value).filter(|url| !url.is_empty()),
            "mpris:length" => {
                track.length = integer(value)
                    .filter(|&length| length > 0)
                    .map(duration_from_micros);
            }
            _ => {}
        }
    }
    track
}

fn duration_from_micros(micros: i64) -> Duration {
    Duration::from_micros(micros.max(0).unsigned_abs())
}

/// Values in `a{sv}` arrive wrapped in a variant.
fn inner<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(boxed) => inner(boxed),
        other => other,
    }
}

fn string(value: &Value<'_>) -> Option<String> {
    match inner(value) {
        Value::Str(text) => Some(text.to_string()),
        Value::ObjectPath(path) => Some(path.to_string()),
        _ => None,
    }
}

/// The spec says `as`, but some players send a single string.
fn strings(value: &Value<'_>) -> Vec<String> {
    match inner(value) {
        Value::Array(array) => array
            .iter()
            .filter_map(string)
            .filter(|text| !text.is_empty())
            .collect(),
        other => string(other).into_iter().collect(),
    }
}

/// The spec says `x`, but players send every integer type.
fn integer(value: &Value<'_>) -> Option<i64> {
    match *inner(value) {
        Value::I64(number) => Some(number),
        Value::U64(number) => i64::try_from(number).ok(),
        Value::I32(number) => Some(number.into()),
        Value::U32(number) => Some(number.into()),
        Value::I16(number) => Some(number.into()),
        Value::U16(number) => Some(number.into()),
        Value::U8(number) => Some(number.into()),
        // Rounding is fine: these are microseconds.
        #[allow(clippy::cast_possible_truncation)]
        Value::F64(number) => Some(number as i64),
        _ => None,
    }
}

fn float(value: &Value<'_>) -> Option<f64> {
    match *inner(value) {
        Value::F64(number) => Some(number),
        _ => integer(value).map(|number| number as f64),
    }
}

fn boolean(value: &Value<'_>) -> Option<bool> {
    match *inner(value) {
        Value::Bool(flag) => Some(flag),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(value: Value<'_>) -> OwnedValue {
        value.try_to_owned().unwrap()
    }

    fn properties(metadata: HashMap<&str, Value<'_>>) -> HashMap<String, OwnedValue> {
        let metadata: HashMap<&str, Value<'_>> = metadata
            .into_iter()
            .map(|(key, value)| (key, Value::Value(Box::new(value))))
            .collect();
        HashMap::from([
            ("PlaybackStatus".into(), owned(Value::from("Playing"))),
            ("Metadata".into(), owned(Value::from(metadata))),
            ("Position".into(), owned(Value::from(30_000_000_i64))),
            ("Rate".into(), owned(Value::from(1.0))),
            ("CanGoNext".into(), owned(Value::from(true))),
            ("CanSeek".into(), owned(Value::from(true))),
        ])
    }

    #[test]
    fn reads_a_player() {
        let values = properties(HashMap::from([
            (
                "mpris:trackid",
                Value::from(ObjectPath::try_from("/track/1").unwrap()),
            ),
            ("xesam:title", Value::from("Song")),
            ("xesam:artist", Value::from(vec!["A", "B"])),
            ("xesam:album", Value::from("Album")),
            ("mpris:artUrl", Value::from("https://example.com/a.jpg")),
            ("mpris:length", Value::from(200_000_000_u64)),
        ]));
        let player = parse("Spotify".into(), &values, SystemTime::UNIX_EPOCH);

        assert_eq!(player.status, Status::Playing);
        assert_eq!(player.position, Some(Duration::from_secs(30)));
        assert!(player.can_next && player.can_seek && !player.can_previous);
        assert_eq!(
            player.track,
            Track {
                id: Some("/track/1".into()),
                title: "Song".into(),
                artists: vec!["A".into(), "B".into()],
                album: "Album".into(),
                art: Some("https://example.com/a.jpg".into()),
                length: Some(Duration::from_secs(200)),
            }
        );
    }

    #[test]
    fn tolerates_odd_players() {
        let values = properties(HashMap::from([
            ("mpris:trackid", Value::from(NO_TRACK)),
            ("xesam:artist", Value::from("Solo")),
            ("mpris:length", Value::from(5_000_000_i32)),
            ("mpris:artUrl", Value::from("")),
        ]));
        let track = parse(String::new(), &values, SystemTime::UNIX_EPOCH).track;
        assert_eq!(track.id, None);
        assert_eq!(track.artists, ["Solo"]);
        assert_eq!(track.length, Some(Duration::from_secs(5)));
        assert_eq!(track.art, None);

        let empty = parse(String::new(), &HashMap::new(), SystemTime::UNIX_EPOCH);
        assert_eq!(empty.status, Status::Stopped);
        assert_eq!(empty.position, None);
        assert_eq!(empty.rate, 1.0);
    }

    #[test]
    fn position_moves_only_while_playing() {
        let start = SystemTime::UNIX_EPOCH;
        let values = properties(HashMap::from([(
            "mpris:length",
            Value::from(40_000_000_i64),
        )]));
        let mut player = parse(String::new(), &values, start);
        let later = start + Duration::from_secs(4);
        assert_eq!(player.position_at(later), Some(Duration::from_secs(34)));
        // Never past the end of the track.
        let much_later = start + Duration::from_secs(60);
        assert_eq!(
            player.position_at(much_later),
            Some(Duration::from_secs(40))
        );

        player.status = Status::Paused;
        assert_eq!(player.position_at(later), Some(Duration::from_secs(30)));
    }

    #[test]
    fn short_names_drop_the_instance() {
        assert_eq!(short_name("org.mpris.MediaPlayer2.spotify"), "spotify");
        assert_eq!(
            short_name("org.mpris.MediaPlayer2.firefox.instance_1_35"),
            "firefox"
        );
        assert!(!is_player(PLAYERCTLD));
        assert!(is_player("org.mpris.MediaPlayer2.vlc"));
        assert!(!is_player("org.freedesktop.Notifications"));
    }
}

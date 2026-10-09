//! What the mixer shows, and how commands name what they change.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::pulse::{Device, Snapshot, Stream, Target};

/// Before an app's name or a stream's id, names what the app records rather
/// than what it plays: `recording:discord`.
pub const RECORDING: &str = "recording:";

/// Finds what a command names: `output` or `input` for the defaults, an
/// app's stream id, a device's name, or an app's name for all its streams;
/// after `recording:`, an app's recording streams.
pub fn targets(snapshot: &Snapshot, name: &str) -> Result<Vec<Target>, String> {
    if let Some(name) = name.strip_prefix(RECORDING) {
        return pick(&snapshot.recording, name)
            .map(|streams| streams.into_iter().map(Target::Recording).collect())
            .map_err(|_| format!("no app called {name} is recording"));
    }
    match name {
        "output" => snapshot
            .default_sink
            .clone()
            .map(|sink| vec![Target::Sink(sink)])
            .ok_or_else(|| "there is no output".to_owned()),
        "input" => snapshot
            .default_source
            .clone()
            .map(|source| vec![Target::Source(source)])
            .ok_or_else(|| "there is no input".to_owned()),
        _ => {
            if snapshot.sinks.iter().any(|sink| sink.name == name) {
                return Ok(vec![Target::Sink(name.to_owned())]);
            }
            if snapshot.sources.iter().any(|source| source.name == name) {
                return Ok(vec![Target::Source(name.to_owned())]);
            }
            streams(snapshot, name)
                .map(|streams| streams.into_iter().map(Target::Stream).collect())
                .map_err(|_| format!("no output, input or app called {name}"))
        }
    }
}

/// The streams of an app playing sound, by a stream's id or the app's name.
pub fn streams(snapshot: &Snapshot, name: &str) -> Result<Vec<u32>, String> {
    pick(&snapshot.streams, name)
}

/// The streams in `streams` with a volume to change, by a stream's id or
/// the app's name, matched exactly first, then ignoring case.
fn pick(streams: &[Stream], name: &str) -> Result<Vec<u32>, String> {
    let streams = || streams.iter().filter(|stream| stream.mixable);
    if let Ok(index) = name.parse::<u32>()
        && streams().any(|stream| stream.index == index)
    {
        return Ok(vec![index]);
    }
    let named = |exact: bool| -> Vec<u32> {
        streams()
            .filter(|stream| {
                if exact {
                    stream.app == name
                } else {
                    stream.app.eq_ignore_ascii_case(name)
                }
            })
            .map(|stream| stream.index)
            .collect()
    };
    let exact = named(true);
    let found = if exact.is_empty() {
        named(false)
    } else {
        exact
    };
    if found.is_empty() {
        Err(format!("no app called {name}"))
    } else {
        Ok(found)
    }
}

/// The output a name picks for `move`: its name, its description ignoring
/// case, or `output` for the one in use.
pub fn sink<'a>(snapshot: &'a Snapshot, name: &str) -> Result<&'a str, String> {
    let name = match name {
        "output" => snapshot.default_sink.as_deref().unwrap_or_default(),
        name => name,
    };
    snapshot
        .sinks
        .iter()
        .find(|sink| sink.name == name)
        .or_else(|| {
            snapshot
                .sinks
                .iter()
                .find(|sink| sink.description.eq_ignore_ascii_case(name))
        })
        .map(|sink| sink.name.as_str())
        .ok_or_else(|| format!("no output called {name}"))
}

/// The volume and mute of what `targets` names, now: the loudest volume,
/// and muted when all of them are.
pub fn current(snapshot: &Snapshot, targets: &[Target]) -> Option<(u32, bool)> {
    let device = |devices: &[Device], name: &str| {
        devices
            .iter()
            .find(|device| device.name == name)
            .map(|device| (device.volume, device.muted))
    };
    let stream = |streams: &[Stream], index: u32| {
        streams
            .iter()
            .find(|stream| stream.index == index)
            .map(|stream| (stream.volume, stream.muted))
    };
    let mut all: Option<(u32, bool)> = None;
    for target in targets {
        let (volume, muted) = match target {
            Target::Sink(name) => device(&snapshot.sinks, name),
            Target::Source(name) => device(&snapshot.sources, name),
            Target::Stream(index) => stream(&snapshot.streams, *index),
            Target::Recording(index) => stream(&snapshot.recording, *index),
        }?;
        all = Some(all.map_or((volume, muted), |(loudest, all_muted)| {
            (loudest.max(volume), all_muted && muted)
        }));
    }
    all
}

/// Reads a level: `40` sets it, `+5` and `-5` move it from `now`. The result
/// stays within 0 and `max`.
pub fn level(text: &str, now: u32, max: u32) -> Result<u32, String> {
    let text = text.trim().trim_end_matches('%');
    let invalid = || format!("{text} is not a volume; give a percent like 40, +5 or -5");
    let value = if let Some(up) = text.strip_prefix('+') {
        now.saturating_add(up.parse().map_err(|_| invalid())?)
    } else if let Some(down) = text.strip_prefix('-') {
        now.saturating_sub(down.parse().map_err(|_| invalid())?)
    } else {
        text.parse().map_err(|_| invalid())?
    };
    Ok(value.min(max))
}

/// The streams of one app, which share a row in the mixer.
#[derive(Debug, PartialEq)]
pub struct Group<'a> {
    pub app: &'a str,
    pub streams: Vec<&'a Stream>,
}

impl Group<'_> {
    /// Whether any of its streams plays or records, rather than waiting.
    fn active(&self) -> bool {
        self.streams.iter().any(|stream| !stream.corked)
    }
}

/// The streams with a volume, by app name: apps playing or recording first,
/// then by name, and in each, streams playing first. A stream keeps its
/// place while it plays.
pub fn groups(streams: &[Stream]) -> Vec<Group<'_>> {
    let mut groups: Vec<Group<'_>> = Vec::new();
    for stream in streams.iter().filter(|stream| stream.mixable) {
        match groups.iter_mut().find(|group| group.app == stream.app) {
            Some(group) => group.streams.push(stream),
            None => groups.push(Group {
                app: &stream.app,
                streams: vec![stream],
            }),
        }
    }
    for group in &mut groups {
        group
            .streams
            .sort_by_key(|stream| (stream.corked, stream.index));
    }
    groups.sort_by_cached_key(|group| (!group.active(), group.app.to_lowercase()));
    groups
}

/// What the views get. `None` while the audio server isn't connected.
pub fn payload(snapshot: Option<&Snapshot>, max_volume: u32) -> Value {
    let Some(snapshot) = snapshot else {
        return json!({ "connected": false, "max_volume": max_volume });
    };
    let devices = |devices: &[Device], default: Option<&str>| -> Vec<Value> {
        devices
            .iter()
            .map(|device| {
                json!({
                    "name": device.name,
                    "description": device.description,
                    "volume": device.volume,
                    "muted": device.muted,
                    "icon": device.kind,
                    "default": Some(device.name.as_str()) == default,
                })
            })
            .collect()
    };
    let outputs = devices(&snapshot.sinks, snapshot.default_sink.as_deref());
    let inputs = devices(&snapshot.sources, snapshot.default_source.as_deref());
    let chosen = |devices: &[Value]| {
        devices
            .iter()
            .find(|device| device["default"] == true)
            .cloned()
            .unwrap_or(Value::Null)
    };
    let output = |stream: &Stream| {
        snapshot
            .sinks
            .iter()
            .find(|sink| sink.index == stream.device)
            .map(|sink| sink.name.as_str())
    };

    let apps: Vec<Value> = groups(&snapshot.streams)
        .into_iter()
        .map(|group| {
            let mut row = row(&group, "", |stream, value| {
                value["playing"] = (!stream.corked).into();
                value["output"] = output(stream).into();
            });
            // The output they all play through, if they agree.
            let first = output(group.streams[0]);
            let shared =
                first.filter(|_| group.streams.iter().all(|stream| output(stream) == first));
            row["playing"] = group.active().into();
            row["output"] = shared.into();
            row
        })
        .collect();

    // The apps recording, in rows like the apps playing; `target` names them
    // for `volume` and `mute`.
    let recorders: Vec<Value> = groups(&snapshot.recording)
        .into_iter()
        .map(|group| {
            let mut row = row(&group, RECORDING, |stream, value| {
                value["recording"] = (!stream.corked).into();
            });
            row["recording"] = group.active().into();
            row
        })
        .collect();

    // Each app recording from a microphone once, for the privacy dot.
    let mut recording: Vec<Value> = Vec::new();
    for recorder in snapshot
        .recording
        .iter()
        .filter(|recorder| !recorder.corked)
    {
        if !recording
            .iter()
            .any(|app| app["name"] == recorder.app.as_str())
        {
            recording.push(json!({ "name": recorder.app, "icon": recorder.icon }));
        }
    }

    json!({
        "connected": true,
        "max_volume": max_volume,
        "recording": recording,
        "output": chosen(&outputs),
        "input": chosen(&inputs),
        "outputs": outputs,
        "inputs": inputs,
        "apps": apps,
        "recorders": recorders,
    })
}

/// An app's row: its name, icon, volume and mute, and each of its streams,
/// to which `each` adds. `prefix` goes before the names in `target`, what
/// `volume` and `mute` take.
fn row(group: &Group<'_>, prefix: &str, each: impl Fn(&Stream, &mut Value)) -> Value {
    let first = group.streams[0];
    let streams: Vec<Value> = group
        .streams
        .iter()
        .map(|stream| {
            let mut value = json!({
                "id": stream.index.to_string(),
                "target": format!("{prefix}{}", stream.index),
                "title": stream.title,
                "volume": stream.volume,
                "muted": stream.muted,
            });
            each(stream, &mut value);
            value
        })
        .collect();
    json!({
        "id": group.app,
        "target": format!("{prefix}{}", group.app),
        "name": group.app,
        "title": if group.streams.len() == 1 { first.title.as_str() } else { "" },
        "icon": first.icon,
        "volume": group.streams.iter().map(|stream| stream.volume).max(),
        "muted": group.streams.iter().all(|stream| stream.muted),
        "streams": streams,
    })
}

/// What the meters show: `output` and `input`, each stream by id under
/// `streams`, and each app, the loudest of its streams, under `apps`; the
/// same for the apps recording under `recording`. Levels are on the
/// volume's scale, 1 for 100%, so a full-scale sound at 50% fills half of a
/// slider that ends at 100%, and never goes past the volume.
pub fn levels(snapshot: &Snapshot, shown: &HashMap<String, f32>) -> Value {
    let level = |key: &str| shown.get(key).copied().unwrap_or(0.0);
    let round = |level: f32| (f64::from(level) * 100.0).round() / 100.0;
    let mut streams = Map::new();
    let mut apps = Map::new();
    for group in groups(&snapshot.streams) {
        let mut loudest = 0.0f32;
        for stream in group.streams {
            let id = stream.index.to_string();
            let stream_level = level(&id);
            loudest = loudest.max(stream_level);
            streams.insert(id, round(stream_level).into());
        }
        apps.insert(group.app.to_owned(), round(loudest).into());
    }
    // An output's monitor hears the sound before the output's volume, so
    // the meter applies it, as the apps' meters do.
    let output = snapshot
        .sinks
        .iter()
        .find(|sink| snapshot.default_sink.as_ref() == Some(&sink.name))
        .map_or(0.0, |sink| {
            if sink.muted {
                0.0
            } else {
                level("output") * sink.volume as f32 / 100.0
            }
        });

    // A recording stream has no meter of its own: it takes what the input
    // hears, at its own volume. Only the input in use has a meter.
    let input = snapshot
        .sources
        .iter()
        .find(|source| snapshot.default_source.as_ref() == Some(&source.name))
        .map(|source| source.index);
    let mut recording_streams = Map::new();
    let mut recording_apps = Map::new();
    for group in groups(&snapshot.recording) {
        let mut loudest = 0.0f32;
        for stream in group.streams {
            let heard = if stream.muted || stream.corked || Some(stream.device) != input {
                0.0
            } else {
                level("input") * stream.volume as f32 / 100.0
            };
            loudest = loudest.max(heard);
            recording_streams.insert(stream.index.to_string(), round(heard).into());
        }
        recording_apps.insert(group.app.to_owned(), round(loudest).into());
    }

    json!({
        "output": round(output),
        "input": round(level("input")),
        "streams": streams,
        "apps": apps,
        "recording": {
            "streams": recording_streams,
            "apps": recording_apps,
        },
    })
}

/// A meter's next level: a new peak shows on the volume's scale, the cube
/// root, as volumes are cubic; without one, the last level falls by half.
pub fn fall(last: f32, peak: Option<f32>) -> f32 {
    match peak {
        Some(peak) => peak.clamp(0.0, 1.0).cbrt(),
        None if last < 0.02 => 0.0,
        None => last / 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(index: u32, name: &str, volume: u32) -> Device {
        Device {
            index,
            name: name.into(),
            description: format!("{name} device"),
            volume,
            muted: false,
            kind: "speakers",
            monitor: Some(format!("{name}.monitor")),
            running: true,
        }
    }

    fn stream(index: u32, app: &str, corked: bool) -> Stream {
        Stream {
            index,
            device: 1,
            app: app.into(),
            title: format!("{app} {index}"),
            icon: None,
            volume: 80,
            muted: false,
            corked,
            mixable: true,
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            default_sink: Some("speakers".into()),
            default_source: Some("mic".into()),
            sinks: vec![device(1, "speakers", 40), device(2, "headset", 70)],
            sources: vec![device(3, "mic", 100)],
            streams: vec![
                stream(12, "Spotify", true),
                stream(30, "Firefox", false),
                stream(7, "discord", false),
                Stream {
                    volume: 50,
                    muted: true,
                    device: 2,
                    ..stream(31, "Firefox", true)
                },
                Stream {
                    muted: true,
                    ..stream(5, "Firefox", false)
                },
            ],
            // Recording streams count apart from playing ones: 12 is
            // Spotify's too.
            recording: vec![
                recorder(12, "discord", false),
                Stream {
                    volume: 50,
                    ..recorder(13, "discord", false)
                },
                recorder(4, "OBS", true),
                // Counts as recording, but has no volume to mix.
                Stream {
                    mixable: false,
                    ..recorder(9, "Speech", false)
                },
            ],
        }
    }

    fn recorder(index: u32, app: &str, corked: bool) -> Stream {
        Stream {
            device: 3,
            volume: 100,
            ..stream(index, app, corked)
        }
    }

    #[test]
    fn names_what_a_command_changes() {
        let snapshot = snapshot();
        assert_eq!(
            targets(&snapshot, "output"),
            Ok(vec![Target::Sink("speakers".into())])
        );
        assert_eq!(
            targets(&snapshot, "input"),
            Ok(vec![Target::Source("mic".into())])
        );
        assert_eq!(targets(&snapshot, "12"), Ok(vec![Target::Stream(12)]));
        assert_eq!(
            targets(&snapshot, "headset"),
            Ok(vec![Target::Sink("headset".into())])
        );
        assert_eq!(
            targets(&snapshot, "Firefox"),
            Ok(vec![
                Target::Stream(30),
                Target::Stream(31),
                Target::Stream(5)
            ])
        );
        assert_eq!(targets(&snapshot, "spotify"), Ok(vec![Target::Stream(12)]));
        assert!(targets(&snapshot, "13").is_err());
        assert!(targets(&Snapshot::default(), "output").is_err());

        assert_eq!(
            current(&snapshot, &[Target::Sink("headset".into())]),
            Some((70, false))
        );
        assert_eq!(current(&snapshot, &[Target::Stream(30)]), Some((80, false)));
    }

    #[test]
    fn names_what_an_app_records() {
        let snapshot = snapshot();
        assert_eq!(
            targets(&snapshot, "recording:Discord"),
            Ok(vec![Target::Recording(12), Target::Recording(13)])
        );
        // An id names the recording stream, not the playing one.
        assert_eq!(
            targets(&snapshot, "recording:12"),
            Ok(vec![Target::Recording(12)])
        );
        assert_eq!(
            current(&snapshot, &[Target::Recording(13)]),
            Some((50, false))
        );
        // Spotify only plays, and Speech has no volume to change.
        assert!(targets(&snapshot, "recording:spotify").is_err());
        assert!(targets(&snapshot, "recording:Speech").is_err());
        assert!(targets(&snapshot, "recording:9").is_err());
        // Without the prefix, apps that only record aren't found.
        assert!(targets(&snapshot, "OBS").is_err());
    }

    #[test]
    fn an_app_is_as_loud_as_its_loudest_stream_and_muted_when_all_are() {
        let snapshot = snapshot();
        let firefox = targets(&snapshot, "Firefox").unwrap();
        assert_eq!(current(&snapshot, &firefox), Some((80, false)));
        let quiet = [Target::Stream(31), Target::Stream(5)];
        assert_eq!(current(&snapshot, &quiet), Some((80, true)));
        assert_eq!(current(&snapshot, &[Target::Stream(99)]), None);
    }

    #[test]
    fn picks_an_output_to_move_to() {
        let snapshot = snapshot();
        assert_eq!(sink(&snapshot, "headset"), Ok("headset"));
        assert_eq!(sink(&snapshot, "HEADSET device"), Ok("headset"));
        assert_eq!(sink(&snapshot, "output"), Ok("speakers"));
        assert!(sink(&snapshot, "mic").is_err());
        assert_eq!(streams(&snapshot, "7"), Ok(vec![7]));
        assert!(streams(&snapshot, "speakers").is_err());
    }

    #[test]
    fn groups_streams_by_app() {
        let snapshot = snapshot();
        let groups = groups(&snapshot.streams);
        let apps: Vec<&str> = groups.iter().map(|group| group.app).collect();
        // Spotify only has a paused stream, so it comes last.
        assert_eq!(apps, ["discord", "Firefox", "Spotify"]);
        let firefox: Vec<u32> = groups[1]
            .streams
            .iter()
            .map(|stream| stream.index)
            .collect();
        assert_eq!(firefox, [5, 30, 31]);
    }

    #[test]
    fn reads_levels_and_steps() {
        assert_eq!(level("40", 10, 100), Ok(40));
        assert_eq!(level("40%", 10, 100), Ok(40));
        assert_eq!(level("+5", 98, 100), Ok(100));
        assert_eq!(level("+5", 98, 150), Ok(103));
        assert_eq!(level("-5", 3, 100), Ok(0));
        assert_eq!(level("200", 0, 150), Ok(150));
        assert_eq!(level("+10", 195, 200), Ok(200));
        assert!(level("loud", 0, 100).is_err());
        assert!(level("+", 0, 100).is_err());
    }

    #[test]
    fn payload_lists_the_defaults_and_groups_apps() {
        let payload = payload(Some(&snapshot()), 100);
        assert_eq!(payload["output"]["name"], "speakers");
        assert_eq!(payload["output"]["volume"], 40);
        assert_eq!(payload["outputs"][1]["default"], false);
        assert_eq!(payload["input"]["name"], "mic");
        let names: Vec<&str> = payload["apps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|app| app["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["discord", "Firefox", "Spotify"]);
        assert_eq!(payload["apps"][2]["playing"], false);

        let discord = &payload["apps"][0];
        assert_eq!(discord["id"], "discord");
        assert_eq!(discord["title"], "discord 7");
        assert_eq!(discord["output"], "speakers");
        assert_eq!(discord["streams"][0]["id"], "7");

        let firefox = &payload["apps"][1];
        assert_eq!(firefox["streams"].as_array().unwrap().len(), 3);
        assert_eq!(firefox["title"], "");
        assert_eq!(firefox["volume"], 80);
        assert_eq!(firefox["muted"], false);
        assert_eq!(firefox["playing"], true);
        // One of its streams plays through the headset.
        assert_eq!(firefox["output"], Value::Null);
        assert_eq!(firefox["streams"][2]["output"], "headset");

        assert_eq!(discord["target"], "discord");
        assert_eq!(discord["streams"][0]["target"], "7");

        // Each app recording once, and none paused, with a volume or not.
        assert_eq!(
            payload["recording"],
            json!([{ "name": "discord", "icon": null }, { "name": "Speech", "icon": null }])
        );

        assert_eq!(super::payload(None, 100)["connected"], false);
    }

    #[test]
    fn payload_lists_the_apps_recording() {
        let payload = payload(Some(&snapshot()), 100);
        let recorders = payload["recorders"].as_array().unwrap();
        // Only those with a volume, recording first.
        let names: Vec<&str> = recorders
            .iter()
            .map(|app| app["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["discord", "OBS"]);

        let discord = &recorders[0];
        assert_eq!(discord["target"], "recording:discord");
        assert_eq!(discord["volume"], 100);
        assert_eq!(discord["muted"], false);
        assert_eq!(discord["recording"], true);
        assert_eq!(discord["streams"][1]["target"], "recording:13");
        assert_eq!(discord["streams"][1]["volume"], 50);
        assert_eq!(recorders[1]["recording"], false);
    }

    #[test]
    fn an_app_meters_its_loudest_stream() {
        let shown = HashMap::from([
            ("output".to_owned(), 0.5),
            ("30".to_owned(), 0.25),
            ("5".to_owned(), 0.75),
        ]);
        let levels = levels(&snapshot(), &shown);
        // The output is at 40%.
        assert_eq!(levels["output"], 0.2);
        assert_eq!(levels["input"], 0.0);
        assert_eq!(levels["streams"]["30"], 0.25);
        assert_eq!(levels["streams"]["31"], 0.0);
        assert_eq!(levels["apps"]["Firefox"], 0.75);
        assert_eq!(levels["apps"]["discord"], 0.0);
    }

    #[test]
    fn an_app_recording_meters_the_input_at_its_volume() {
        let shown = HashMap::from([("input".to_owned(), 0.5)]);
        let mut snapshot = snapshot();
        let levels = levels(&snapshot, &shown);
        let recording = &levels["recording"];
        assert_eq!(recording["streams"]["12"], 0.5);
        assert_eq!(recording["streams"]["13"], 0.25);
        assert_eq!(recording["apps"]["discord"], 0.5);
        // Paused.
        assert_eq!(recording["apps"]["OBS"], 0.0);

        // Muted, or recording from another input than the one metered.
        snapshot.recording[0].muted = true;
        snapshot.recording[1].device = 8;
        let levels = super::levels(&snapshot, &shown);
        assert_eq!(levels["recording"]["apps"]["discord"], 0.0);
    }

    #[test]
    fn meters_show_the_cube_root_and_fall_when_quiet() {
        assert!((fall(0.0, Some(0.125)) - 0.5).abs() < 1e-6);
        assert!((fall(0.6, None) - 0.3).abs() < 1e-6);
        assert!(fall(0.01, None).abs() < f32::EPSILON);
        assert!((fall(0.0, Some(2.0)) - 1.0).abs() < 1e-6);
    }
}

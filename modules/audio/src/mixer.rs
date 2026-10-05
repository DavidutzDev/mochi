//! What the mixer shows, and how commands name what they change.

use serde_json::{Value, json};

use crate::pulse::{Device, Snapshot, Target};

/// Finds what a command names: `output` or `input` for the defaults, an
/// app's stream id, or a device's name.
pub fn target(snapshot: &Snapshot, name: &str) -> Result<Target, String> {
    match name {
        "output" => snapshot
            .default_sink
            .clone()
            .map(Target::Sink)
            .ok_or_else(|| "there is no output".to_owned()),
        "input" => snapshot
            .default_source
            .clone()
            .map(Target::Source)
            .ok_or_else(|| "there is no input".to_owned()),
        _ => {
            if let Ok(index) = name.parse::<u32>()
                && snapshot.streams.iter().any(|stream| stream.index == index)
            {
                return Ok(Target::Stream(index));
            }
            if snapshot.sinks.iter().any(|sink| sink.name == name) {
                return Ok(Target::Sink(name.to_owned()));
            }
            if snapshot.sources.iter().any(|source| source.name == name) {
                return Ok(Target::Source(name.to_owned()));
            }
            Err(format!("no output, input or app called {name}"))
        }
    }
}

/// The volume and mute of what `target` names, now.
pub fn current(snapshot: &Snapshot, target: &Target) -> Option<(u32, bool)> {
    let device = |devices: &[Device], name: &str| {
        devices
            .iter()
            .find(|device| device.name == name)
            .map(|device| (device.volume, device.muted))
    };
    match target {
        Target::Sink(name) => device(&snapshot.sinks, name),
        Target::Source(name) => device(&snapshot.sources, name),
        Target::Stream(index) => snapshot
            .streams
            .iter()
            .find(|stream| stream.index == *index)
            .map(|stream| (stream.volume, stream.muted)),
    }
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

    // Apps playing first, then by name; a stream keeps its place while it
    // plays.
    let mut streams: Vec<_> = snapshot.streams.iter().collect();
    streams.sort_by(|a, b| {
        (a.corked, a.app.to_lowercase(), a.index).cmp(&(b.corked, b.app.to_lowercase(), b.index))
    });
    let apps: Vec<Value> = streams
        .into_iter()
        .map(|stream| {
            json!({
                "id": stream.index.to_string(),
                "name": stream.app,
                "title": stream.title,
                "icon": stream.icon,
                "volume": stream.volume,
                "muted": stream.muted,
                "playing": !stream.corked,
            })
        })
        .collect();

    json!({
        "connected": true,
        "max_volume": max_volume,
        "output": chosen(&outputs),
        "input": chosen(&inputs),
        "outputs": outputs,
        "inputs": inputs,
        "apps": apps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse::Stream;

    fn device(name: &str, volume: u32) -> Device {
        Device {
            name: name.into(),
            description: format!("{name} device"),
            volume,
            muted: false,
            kind: "speakers",
        }
    }

    fn stream(index: u32, app: &str, corked: bool) -> Stream {
        Stream {
            index,
            app: app.into(),
            title: String::new(),
            icon: None,
            volume: 80,
            muted: false,
            corked,
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            default_sink: Some("speakers".into()),
            default_source: Some("mic".into()),
            sinks: vec![device("speakers", 40), device("headset", 70)],
            sources: vec![device("mic", 100)],
            streams: vec![
                stream(12, "Spotify", true),
                stream(30, "Firefox", false),
                stream(7, "discord", false),
            ],
        }
    }

    #[test]
    fn names_what_a_command_changes() {
        let snapshot = snapshot();
        assert_eq!(
            target(&snapshot, "output"),
            Ok(Target::Sink("speakers".into()))
        );
        assert_eq!(target(&snapshot, "input"), Ok(Target::Source("mic".into())));
        assert_eq!(target(&snapshot, "12"), Ok(Target::Stream(12)));
        assert_eq!(
            target(&snapshot, "headset"),
            Ok(Target::Sink("headset".into()))
        );
        assert!(target(&snapshot, "13").is_err());
        assert!(target(&Snapshot::default(), "output").is_err());

        assert_eq!(
            current(&snapshot, &Target::Sink("headset".into())),
            Some((70, false))
        );
        assert_eq!(current(&snapshot, &Target::Stream(30)), Some((80, false)));
    }

    #[test]
    fn reads_levels_and_steps() {
        assert_eq!(level("40", 10, 100), Ok(40));
        assert_eq!(level("40%", 10, 100), Ok(40));
        assert_eq!(level("+5", 98, 100), Ok(100));
        assert_eq!(level("+5", 98, 150), Ok(103));
        assert_eq!(level("-5", 3, 100), Ok(0));
        assert_eq!(level("200", 0, 150), Ok(150));
        assert!(level("loud", 0, 100).is_err());
        assert!(level("+", 0, 100).is_err());
    }

    #[test]
    fn payload_lists_the_defaults_and_playing_apps_first() {
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
        assert_eq!(payload["apps"][0]["id"], "7");

        assert_eq!(super::payload(None, 100)["connected"], false);
    }
}

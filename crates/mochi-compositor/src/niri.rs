//! niri's IPC, at `$NIRI_SOCKET`: the focused output, where windows are,
//! and every screencast, which its event stream reports from the start.
//!
//! A request is a JSON line, like `"Windows"`, and its reply one line,
//! `{"Ok": ...}` or `{"Err": "..."}`. After `"EventStream"`, niri writes
//! one event a line, starting with the whole state.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::UnboundedSender;

use crate::Window;
use crate::ipc::Event;

/// Sends `request` and returns what its `Ok` holds.
async fn request(path: &Path, request: &str) -> std::io::Result<Value> {
    let mut socket = UnixStream::connect(path).await?;
    socket
        .write_all(format!("\"{request}\"\n").as_bytes())
        .await?;
    let mut line = String::new();
    BufReader::new(socket).read_line(&mut line).await?;
    ok(&line)
}

fn ok(line: &str) -> std::io::Result<Value> {
    let reply: Value = serde_json::from_str(line).map_err(std::io::Error::other)?;
    match reply.get("Ok") {
        Some(value) => Ok(value.clone()),
        None => Err(std::io::Error::other(format!(
            "niri said: {}",
            reply["Err"]
        ))),
    }
}

/// What the event stream has said so far, to answer the events that only
/// name an id.
#[derive(Debug, Default)]
struct Tracker {
    /// Each workspace's output, by id.
    outputs: HashMap<u64, String>,
    /// Each screencast's target, by stream id.
    casts: HashMap<u64, String>,
}

impl Tracker {
    /// The events for Mochi in one of niri's.
    fn event(&mut self, event: &Value) -> Vec<Event> {
        let Some((name, body)) = event.as_object().and_then(|map| map.iter().next()) else {
            return Vec::new();
        };
        match name.as_str() {
            "WorkspacesChanged" => {
                let workspaces = body["workspaces"].as_array().cloned().unwrap_or_default();
                self.outputs = workspaces
                    .iter()
                    .filter_map(|workspace| {
                        Some((
                            workspace["id"].as_u64()?,
                            workspace["output"].as_str()?.to_owned(),
                        ))
                    })
                    .collect();
                workspaces
                    .iter()
                    .find(|workspace| workspace["is_focused"] == true)
                    .and_then(|workspace| workspace["output"].as_str())
                    .map(|output| vec![Event::Focus(output.to_owned())])
                    .unwrap_or_default()
            }
            "WorkspaceActivated" if body["focused"] == true => body["id"]
                .as_u64()
                .and_then(|id| self.outputs.get(&id))
                .map(|output| vec![Event::Focus(output.clone())])
                .unwrap_or_default(),
            "CastsChanged" => {
                self.casts = body["casts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|cast| Some((cast["stream_id"].as_u64()?, target(cast))))
                    .collect();
                vec![self.casting()]
            }
            "CastStartedOrChanged" => {
                let cast = &body["cast"];
                if let Some(id) = cast["stream_id"].as_u64() {
                    self.casts.insert(id, target(cast));
                }
                vec![self.casting()]
            }
            "CastStopped" => {
                if let Some(id) = body["stream_id"].as_u64() {
                    self.casts.remove(&id);
                }
                vec![self.casting()]
            }
            _ => Vec::new(),
        }
    }

    fn casting(&self) -> Event {
        let mut targets: Vec<String> = self.casts.values().cloned().collect();
        targets.sort();
        Event::Casts(targets)
    }
}

/// What a screencast captures: an output's name, a window, or nothing yet
/// for a dynamic target.
fn target(cast: &Value) -> String {
    let target = &cast["target"];
    if let Some(name) = target["Output"]["name"].as_str() {
        name.to_owned()
    } else if let Some(id) = target["Window"]["id"].as_u64() {
        format!("window {id}")
    } else {
        String::new()
    }
}

/// One connection: the whole state, then every change. `Ok` means the
/// receiver is gone.
pub(crate) async fn session(path: &Path, events: &UnboundedSender<Event>) -> std::io::Result<()> {
    let mut socket = UnixStream::connect(path).await?;
    socket.write_all(b"\"EventStream\"\n").await?;
    let mut lines = BufReader::new(socket).lines();
    let reply = lines.next_line().await?.unwrap_or_default();
    ok(&reply)?;
    if events.send(Event::Connected).is_err() {
        return Ok(());
    }
    let mut tracker = Tracker::default();
    while let Some(line) = lines.next_line().await? {
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        for event in tracker.event(&event) {
            if events.send(event).is_err() {
                return Ok(());
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "niri's event stream closed",
    ))
}

/// The windows on active workspaces, topmost first.
pub(crate) async fn windows(path: &Path) -> std::io::Result<Vec<Window>> {
    let windows = request(path, "Windows").await?;
    let workspaces = request(path, "Workspaces").await?;
    let outputs = request(path, "Outputs").await?;
    Ok(windows_from(
        &windows["Windows"],
        &workspaces["Workspaces"],
        &outputs["Outputs"],
    ))
}

/// The windows on each output's active workspace, in the global layout:
/// floating ones first, then each by how recently it had focus. niri
/// doesn't report its stacking order.
fn windows_from(windows: &Value, workspaces: &Value, outputs: &Value) -> Vec<Window> {
    let shown: HashMap<u64, &str> = workspaces
        .as_array()
        .into_iter()
        .flatten()
        .filter(|workspace| workspace["is_active"] == true)
        .filter_map(|workspace| Some((workspace["id"].as_u64()?, workspace["output"].as_str()?)))
        .collect();
    let pair = |value: &Value| Some((value[0].as_f64()?, value[1].as_f64()?));
    let mut found: Vec<(bool, i64, Window)> = windows
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|window| {
            let output = shown.get(&window["workspace_id"].as_u64()?)?;
            let logical = &outputs[*output]["logical"];
            let (left, top) = (logical["x"].as_f64()?, logical["y"].as_f64()?);
            let layout = &window["layout"];
            let (tile_x, tile_y) = pair(&layout["tile_pos_in_workspace_view"])?;
            let (offset_x, offset_y) = pair(&layout["window_offset_in_tile"]).unwrap_or_default();
            let (width, height) = pair(&layout["window_size"])?;
            let floating = window["is_floating"] == true;
            let focused = window["focus_timestamp"]["secs"].as_i64().unwrap_or(0);
            Some((
                floating,
                focused,
                Window {
                    title: window["title"].as_str().unwrap_or_default().to_owned(),
                    app_id: window["app_id"].as_str().unwrap_or_default().to_owned(),
                    x: (left + tile_x + offset_x).round() as i32,
                    y: (top + tile_y + offset_y).round() as i32,
                    width: width as i32,
                    height: height as i32,
                    floating,
                },
            ))
        })
        .collect();
    found.sort_by_key(|(floating, focused, _)| (!floating, -focused));
    found.into_iter().map(|(_, _, window)| window).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn replies_unwrap_or_say_the_error() {
        assert_eq!(ok(r#"{"Ok":"Handled"}"#).unwrap(), json!("Handled"));
        let error = ok(r#"{"Err":"no such output"}"#).unwrap_err().to_string();
        assert!(error.contains("no such output"), "{error}");
    }

    #[test]
    fn the_focus_follows_workspaces() {
        let mut tracker = Tracker::default();
        let events = tracker.event(&json!({ "WorkspacesChanged": { "workspaces": [
            { "id": 1, "output": "DP-1", "is_active": true, "is_focused": false },
            { "id": 2, "output": "HDMI-A-1", "is_active": true, "is_focused": true },
            { "id": 3, "output": "DP-1", "is_active": false, "is_focused": false },
        ] } }));
        assert_eq!(events, [Event::Focus("HDMI-A-1".into())]);
        let events = tracker.event(&json!({ "WorkspaceActivated": { "id": 3, "focused": true } }));
        assert_eq!(events, [Event::Focus("DP-1".into())]);
        let events = tracker.event(&json!({ "WorkspaceActivated": { "id": 1, "focused": false } }));
        assert!(events.is_empty());
    }

    #[test]
    fn screencasts_are_followed() {
        let mut tracker = Tracker::default();
        let cast = |id: u64, target: Value| json!({ "stream_id": id, "session_id": 1, "target": target, "is_active": true });
        let events = tracker.event(&json!({ "CastsChanged": { "casts": [
            cast(4, json!({ "Output": { "name": "DP-1" } })),
        ] } }));
        assert_eq!(events, [Event::Casts(vec!["DP-1".into()])]);
        let events = tracker.event(&json!({ "CastStartedOrChanged": { "cast": cast(5, json!({ "Window": { "id": 9 } })) } }));
        assert_eq!(
            events,
            [Event::Casts(vec!["DP-1".into(), "window 9".into()])]
        );
        let events = tracker.event(&json!({ "CastStopped": { "stream_id": 4 } }));
        assert_eq!(events, [Event::Casts(vec!["window 9".into()])]);
    }

    #[test]
    fn windows_are_placed_on_their_outputs() {
        let windows = json!([
            {
                "id": 1, "title": "Docs", "app_id": "firefox", "workspace_id": 2,
                "is_floating": false, "focus_timestamp": { "secs": 100, "nanos": 0 },
                "layout": {
                    "tile_pos_in_workspace_view": [16.0, 16.0],
                    "window_offset_in_tile": [2.0, 2.0],
                    "window_size": [940, 1040],
                },
            },
            {
                "id": 2, "title": "Clock", "app_id": "clock", "workspace_id": 2,
                "is_floating": true, "focus_timestamp": { "secs": 50, "nanos": 0 },
                "layout": {
                    "tile_pos_in_workspace_view": [500.0, 300.0],
                    "window_offset_in_tile": [0.0, 0.0],
                    "window_size": [300, 200],
                },
            },
            {
                "id": 3, "title": "Hidden", "app_id": "kitty", "workspace_id": 7,
                "is_floating": false,
                "layout": {
                    "tile_pos_in_workspace_view": [0.0, 0.0],
                    "window_offset_in_tile": [0.0, 0.0],
                    "window_size": [100, 100],
                },
            },
        ]);
        let workspaces = json!([
            { "id": 2, "output": "HDMI-A-1", "is_active": true },
            { "id": 7, "output": "HDMI-A-1", "is_active": false },
        ]);
        let outputs = json!({ "HDMI-A-1": { "logical": { "x": 1920, "y": 0, "width": 1920, "height": 1080, "scale": 1.0 } } });
        let found = windows_from(&windows, &workspaces, &outputs);
        let titles: Vec<&str> = found.iter().map(|window| window.title.as_str()).collect();
        assert_eq!(titles, ["Clock", "Docs"]);
        assert_eq!((found[1].x, found[1].y), (1920 + 18, 18));
        assert_eq!((found[0].x, found[0].width), (1920 + 500, 300));
    }
}

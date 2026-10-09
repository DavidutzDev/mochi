//! Sway's IPC, at `$SWAYSOCK`: the focused output, which its workspace
//! events report even when focus moves to an empty workspace, the keyboard
//! layout, from its input events, and where windows are, from its tree. It
//! says nothing about screencasts.
//!
//! Each message is the bytes `i3-ipc`, the payload's length and the
//! message's type as native-endian 32-bit integers, then a JSON payload.
//! Events have the high bit of their type set.

use std::path::Path;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc::UnboundedSender;

use crate::Window;
use crate::ipc::{self, Event};

const MAGIC: &[u8; 6] = b"i3-ipc";
const SUBSCRIBE: u32 = 2;
const GET_OUTPUTS: u32 = 3;
const GET_TREE: u32 = 4;
const GET_INPUTS: u32 = 100;
/// The workspace and input events, with the event bit set.
const WORKSPACE_EVENT: u32 = 0x8000_0000;
const INPUT_EVENT: u32 = 0x8000_0015;

async fn send(socket: &mut UnixStream, kind: u32, payload: &str) -> std::io::Result<()> {
    let mut message = Vec::with_capacity(14 + payload.len());
    message.extend_from_slice(MAGIC);
    message.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
    message.extend_from_slice(&kind.to_ne_bytes());
    message.extend_from_slice(payload.as_bytes());
    socket.write_all(&message).await
}

/// The next message: its type and its JSON.
async fn receive(socket: &mut UnixStream) -> std::io::Result<(u32, Value)> {
    let mut header = [0u8; 14];
    socket.read_exact(&mut header).await?;
    if &header[..6] != MAGIC {
        return Err(std::io::Error::other(
            "Sway sent something that isn't i3-ipc",
        ));
    }
    let length = u32::from_ne_bytes(header[6..10].try_into().expect("4 bytes")) as usize;
    let kind = u32::from_ne_bytes(header[10..14].try_into().expect("4 bytes"));
    let mut payload = vec![0u8; length];
    socket.read_exact(&mut payload).await?;
    let value = serde_json::from_slice(&payload).map_err(std::io::Error::other)?;
    Ok((kind, value))
}

async fn request(path: &Path, kind: u32) -> std::io::Result<Value> {
    let mut socket = UnixStream::connect(path).await?;
    send(&mut socket, kind, "").await?;
    Ok(receive(&mut socket).await?.1)
}

/// One connection: the focused output and the keyboard layout, then every
/// change to them. `Ok` means the receiver is gone.
pub(crate) async fn session(path: &Path, events: &UnboundedSender<Event>) -> std::io::Result<()> {
    // Subscribe first, so no change slips in between the query and the
    // subscription.
    let mut socket = UnixStream::connect(path).await?;
    send(&mut socket, SUBSCRIBE, r#"["workspace", "input"]"#).await?;
    receive(&mut socket).await?;
    if events.send(Event::Connected).is_err() {
        return Ok(());
    }
    if let Some(output) = focused_output(&request(path, GET_OUTPUTS).await?)
        && events.send(Event::Focus(output)).is_err()
    {
        return Ok(());
    }
    if let Some(layout) = layout_from_inputs(&request(path, GET_INPUTS).await?)
        && events.send(Event::KeyboardLayout(layout)).is_err()
    {
        return Ok(());
    }
    loop {
        let (kind, event) = receive(&mut socket).await?;
        let event = match kind {
            WORKSPACE_EVENT => focused_from_event(&event).map(Event::Focus),
            INPUT_EVENT => layout_from_event(&event).map(Event::KeyboardLayout),
            _ => None,
        };
        if let Some(event) = event
            && events.send(event).is_err()
        {
            return Ok(());
        }
    }
}

/// A keyboard's active layout, from `get_inputs` or an input event.
fn layout_of(input: &Value) -> Option<String> {
    if input["type"] != "keyboard" {
        return None;
    }
    ipc::layout_name(input["xkb_active_layout_name"].as_str()?)
}

/// The layout of the first keyboard with a named one. Sway has no main
/// keyboard, and keyboards usually share the layout from its config.
fn layout_from_inputs(inputs: &Value) -> Option<String> {
    inputs.as_array()?.iter().find_map(layout_of)
}

/// An input event for a keyboard that came, or whose layout or keymap
/// changed.
fn layout_from_event(event: &Value) -> Option<String> {
    if event["change"] == "removed" {
        return None;
    }
    layout_of(&event["input"])
}

/// The output `get_outputs` marks as focused.
fn focused_output(outputs: &Value) -> Option<String> {
    outputs
        .as_array()?
        .iter()
        .find(|output| output["focused"] == true)
        .and_then(|output| output["name"].as_str())
        .map(str::to_owned)
}

/// A workspace event that moves the focus: the output of the workspace
/// that has it now.
fn focused_from_event(event: &Value) -> Option<String> {
    if event["change"] != "focus" && event["change"] != "init" && event["change"] != "move" {
        return None;
    }
    let current = &event["current"];
    if current["focused"] == false {
        return None;
    }
    current["output"].as_str().map(str::to_owned)
}

/// The windows on visible workspaces, topmost first.
pub(crate) async fn windows(path: &Path) -> std::io::Result<Vec<Window>> {
    Ok(windows_from(&request(path, GET_TREE).await?))
}

/// The visible windows in Sway's tree, topmost first: fullscreen, then
/// floating, then tiled, the focused one first in each.
fn windows_from(tree: &Value) -> Vec<Window> {
    let mut found = Vec::new();
    collect(tree, false, &mut found);
    found.sort_by_key(|(layer, focused, _)| (*layer, !focused));
    found.into_iter().map(|(_, _, window)| window).collect()
}

fn collect(node: &Value, floating: bool, found: &mut Vec<(u8, bool, Window)>) {
    // A window has a pid; containers and workspaces don't.
    if node["pid"].is_number() && node["visible"] == true {
        let rect = &node["rect"];
        let number = |key: &str| rect[key].as_i64().unwrap_or(0) as i32;
        let floating = floating || node["type"] == "floating_con";
        let layer = if node["fullscreen_mode"].as_i64().unwrap_or(0) > 0 {
            0
        } else if floating {
            1
        } else {
            2
        };
        // Xwayland windows have a class instead of an app id.
        let app_id = node["app_id"]
            .as_str()
            .or_else(|| node["window_properties"]["class"].as_str())
            .unwrap_or_default();
        found.push((
            layer,
            node["focused"] == true,
            Window {
                title: node["name"].as_str().unwrap_or_default().to_owned(),
                app_id: app_id.to_owned(),
                x: number("x"),
                y: number("y"),
                width: number("width"),
                height: number("height"),
                floating,
            },
        ));
    }
    for child in node["nodes"].as_array().into_iter().flatten() {
        collect(child, floating, found);
    }
    for child in node["floating_nodes"].as_array().into_iter().flatten() {
        collect(child, true, found);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn windows_come_from_the_tree() {
        let tree: Value =
            serde_json::from_str(include_str!("../tests/fixtures/sway-tree.json")).unwrap();
        let windows = windows_from(&tree);
        assert_eq!(windows.len(), 2, "{windows:?}");
        // The floating window is over the tiled one, though it lost focus.
        assert!(windows[0].floating);
        assert_eq!(
            (
                windows[0].x,
                windows[0].y,
                windows[0].width,
                windows[0].height
            ),
            (612, 293, 696, 494)
        );
        assert_eq!(windows[0].app_id, "foot");
        assert_eq!(windows[0].title, "Terminal");
        assert!(!windows[1].floating);
        assert_eq!((windows[1].width, windows[1].height), (1920, 1080));
    }

    #[test]
    fn the_layout_follows_input_events() {
        let inputs = json!([
            { "type": "pointer", "name": "Mouse" },
            { "type": "keyboard", "name": "wlr_virtual_keyboard_v1", "xkb_active_layout_name": null },
            { "type": "keyboard", "name": "AT keyboard", "xkb_active_layout_name": "English (US)" },
        ]);
        assert_eq!(layout_from_inputs(&inputs).as_deref(), Some("English (US)"));
        assert_eq!(layout_from_inputs(&json!([])), None);

        let switched = json!({
            "change": "xkb_layout",
            "input": { "type": "keyboard", "xkb_active_layout_name": "German" },
        });
        assert_eq!(layout_from_event(&switched).as_deref(), Some("German"));
        let removed = json!({
            "change": "removed",
            "input": { "type": "keyboard", "xkb_active_layout_name": "German" },
        });
        assert_eq!(layout_from_event(&removed), None);
        let touchpad = json!({ "change": "libinput_config", "input": { "type": "touchpad" } });
        assert_eq!(layout_from_event(&touchpad), None);
    }

    #[test]
    fn the_focus_follows_workspace_events() {
        let outputs = json!([
            { "name": "DP-1", "focused": false },
            { "name": "HDMI-A-1", "focused": true },
        ]);
        assert_eq!(focused_output(&outputs).as_deref(), Some("HDMI-A-1"));
        let event = json!({
            "change": "focus",
            "current": { "name": "3", "output": "DP-1", "focused": true },
        });
        assert_eq!(focused_from_event(&event).as_deref(), Some("DP-1"));
        let renamed = json!({ "change": "rename", "current": { "output": "DP-1" } });
        assert_eq!(focused_from_event(&renamed), None);
    }
}

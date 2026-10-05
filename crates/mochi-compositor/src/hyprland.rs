//! Hyprland's IPC, used only for what the standard protocols don't say: which
//! output has focus, and where windows are. Focusing an empty workspace on
//! another monitor moves no window, so the standard window-based guess can't
//! see it; Hyprland's event socket can.
//!
//! The focused output name goes to the Wayland task, which owns the model.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::UnboundedSender;

use crate::Window;

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(30);
/// The longest a blocking request may take.
const BLOCKING_TIMEOUT: Duration = Duration::from_millis(100);

/// Hyprland's socket directory, when running under Hyprland.
pub(crate) fn socket_dir() -> Option<PathBuf> {
    let instance = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let dir = Path::new(&runtime).join("hypr").join(instance);
    dir.join(".socket2.sock").exists().then_some(dir)
}

/// What Hyprland's events tell the Wayland task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// The output with focus.
    Focus(String),
    /// A screencast started (true) or stopped.
    Screencast(bool),
    /// The same, with what it captures: a monitor's name, or a window's.
    Captured { started: bool, target: String },
    /// The event socket (re)connected: counts kept so far may be stale.
    Connected,
}

/// Sends the focused output now, then every focus or screencast change.
/// Reconnects with backoff if the socket closes. Ends when nobody listens
/// any more.
pub(crate) async fn watch(dir: PathBuf, focus: UnboundedSender<Event>) {
    let mut retry = FIRST_RETRY;
    loop {
        match session(&dir, &focus).await {
            Ok(()) => return,
            Err(error) => {
                tracing::warn!(%error, retry = ?retry, "lost Hyprland's event socket");
                tokio::time::sleep(retry).await;
                retry = (retry * 2).min(MAX_RETRY);
            }
        }
    }
}

/// One connection. `Ok` means the receiver is gone.
async fn session(dir: &Path, focus: &UnboundedSender<Event>) -> std::io::Result<()> {
    // Connect to the events first, so no change slips in between the query
    // and the subscription.
    let events = UnixStream::connect(dir.join(".socket2.sock")).await?;
    if focus.send(Event::Connected).is_err() {
        return Ok(());
    }
    if let Some(output) = query_focused(dir).await?
        && focus.send(Event::Focus(output)).is_err()
    {
        return Ok(());
    }

    let mut lines = BufReader::new(events).lines();
    while let Some(line) = lines.next_line().await? {
        if let Some(event) = event(&line)
            && focus.send(event).is_err()
        {
            return Ok(());
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "the event socket closed",
    ))
}

async fn query_focused(dir: &Path) -> std::io::Result<Option<String>> {
    Ok(focused_from_monitors(&query(dir, "j/monitors").await?))
}

/// One request on the command socket, like `j/clients`.
async fn query(dir: &Path, request: &str) -> std::io::Result<String> {
    let mut socket = UnixStream::connect(dir.join(".socket.sock")).await?;
    socket.write_all(request.as_bytes()).await?;
    let mut reply = String::new();
    socket.read_to_string(&mut reply).await?;
    Ok(reply)
}

/// The windows on visible workspaces, topmost first.
pub(crate) async fn windows(dir: &Path) -> std::io::Result<Vec<Window>> {
    let monitors = query(dir, "j/monitors").await?;
    let clients = query(dir, "j/clients").await?;
    windows_from(&clients, &monitors)
        .ok_or_else(|| std::io::Error::other("Hyprland sent unexpected JSON"))
}

/// Picks the visible windows from `hyprctl -j clients` and orders them
/// topmost first: fullscreen, then floating, then tiled, each by how
/// recently it had focus. Hyprland doesn't report its stacking order, and
/// this matches it for everything but overlapping floating windows that
/// were raised without focus.
fn windows_from(clients: &str, monitors: &str) -> Option<Vec<Window>> {
    let clients: serde_json::Value = serde_json::from_str(clients).ok()?;
    let monitors: serde_json::Value = serde_json::from_str(monitors).ok()?;
    // Older Hyprland has no `visible`; then a window is visible when its
    // workspace is active or special on some monitor.
    let shown: Vec<i64> = monitors
        .as_array()?
        .iter()
        .flat_map(|monitor| {
            [
                monitor["activeWorkspace"]["id"].as_i64(),
                monitor["specialWorkspace"]["id"]
                    .as_i64()
                    .filter(|id| *id != 0),
            ]
        })
        .flatten()
        .collect();

    let mut windows: Vec<(i64, i64, Window)> = clients
        .as_array()?
        .iter()
        .filter(|client| client["mapped"] != false && client["hidden"] != true)
        .filter(|client| match client["visible"].as_bool() {
            Some(visible) => visible,
            None => client["workspace"]["id"]
                .as_i64()
                .is_some_and(|id| shown.contains(&id)),
        })
        .filter_map(|client| {
            let number = |value: &serde_json::Value| value.as_i64().map(|n| n as i32);
            let window = Window {
                title: client["title"].as_str().unwrap_or_default().to_owned(),
                app_id: client["class"].as_str().unwrap_or_default().to_owned(),
                x: number(&client["at"][0])?,
                y: number(&client["at"][1])?,
                width: number(&client["size"][0])?,
                height: number(&client["size"][1])?,
                floating: client["floating"] == true,
            };
            let layer = if client["fullscreen"].as_i64().unwrap_or(0) > 0 {
                0
            } else if window.floating {
                1
            } else {
                2
            };
            let recency = client["focusHistoryID"].as_i64().unwrap_or(i64::MAX);
            Some((layer, recency, window))
        })
        .collect();
    windows.sort_by_key(|(layer, recency, _)| (*layer, *recency));
    Some(windows.into_iter().map(|(_, _, window)| window).collect())
}

/// Makes a headless monitor named `name`, far from the real ones so the
/// pointer never reaches it, then gives the keyboard back to `focused`:
/// Hyprland moves it when a monitor comes. Lua configs set the monitor rule
/// through `eval`, older ones through `keyword`.
pub(crate) async fn create_headless(
    dir: &Path,
    name: &str,
    (width, height, refresh): (u32, u32, u32),
    focused: Option<&str>,
) -> std::io::Result<()> {
    let mode = format!("{width}x{height}@{refresh}");
    let lua = format!(
        "eval hl.monitor({{ output = \"{name}\", mode = \"{mode}\", position = \"{FAR_AWAY}\", scale = 1 }})"
    );
    if !ok(&query(dir, &lua).await?) {
        let legacy = format!("keyword monitor {name},{mode},{FAR_AWAY},1");
        expect_ok(&query(dir, &legacy).await?)?;
    }
    expect_ok(&query(dir, &format!("output create headless {name}")).await?)?;
    refocus(dir, focused).await
}

/// Removes a monitor, then gives the keyboard back to `focused`.
pub(crate) async fn remove_output(
    dir: &Path,
    name: &str,
    focused: Option<&str>,
) -> std::io::Result<()> {
    expect_ok(&query(dir, &format!("output remove {name}")).await?)?;
    refocus(dir, focused).await
}

/// Focuses a monitor, in a Lua config's words or an older one's.
async fn refocus(dir: &Path, output: Option<&str>) -> std::io::Result<()> {
    let Some(output) = output else {
        return Ok(());
    };
    let lua = format!("dispatch hl.dsp.focus({{ monitor = \"{output}\" }})");
    if !ok(&query(dir, &lua).await?) {
        query(dir, &format!("dispatch focusmonitor {output}")).await?;
    }
    Ok(())
}

/// Where virtual monitors go: past any real layout.
const FAR_AWAY: &str = "-20000x-20000";

fn ok(reply: &str) -> bool {
    reply.trim() == "ok"
}

fn expect_ok(reply: &str) -> std::io::Result<()> {
    if ok(reply) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "Hyprland said: {}",
            reply.trim()
        )))
    }
}

/// The monitor under the pointer. Blocking, but bounded: the daemon asks
/// this while it decides where a panel opens, and Hyprland answers in about
/// a millisecond.
pub(crate) fn pointer_output(dir: &Path) -> Option<String> {
    let cursor = query_blocking(dir, "j/cursorpos").ok()?;
    let monitors = query_blocking(dir, "j/monitors").ok()?;
    output_at(&cursor, &monitors)
}

fn query_blocking(dir: &Path, request: &str) -> std::io::Result<String> {
    use std::io::{Read, Write};
    let mut socket = std::os::unix::net::UnixStream::connect(dir.join(".socket.sock"))?;
    socket.set_read_timeout(Some(BLOCKING_TIMEOUT))?;
    socket.set_write_timeout(Some(BLOCKING_TIMEOUT))?;
    socket.write_all(request.as_bytes())?;
    let mut reply = String::new();
    socket.read_to_string(&mut reply)?;
    Ok(reply)
}

/// The monitor holding the point in `hyprctl -j cursorpos`, in the global
/// layout, where a monitor covers its pixel size divided by its scale, with
/// width and height swapped when it's rotated a quarter turn.
fn output_at(cursor: &str, monitors: &str) -> Option<String> {
    let cursor: serde_json::Value = serde_json::from_str(cursor).ok()?;
    let (x, y) = (cursor["x"].as_f64()?, cursor["y"].as_f64()?);
    let monitors: serde_json::Value = serde_json::from_str(monitors).ok()?;
    monitors.as_array()?.iter().find_map(|monitor| {
        let scale = monitor["scale"].as_f64().filter(|scale| *scale > 0.0)?;
        let (mut width, mut height) = (
            monitor["width"].as_f64()? / scale,
            monitor["height"].as_f64()? / scale,
        );
        if monitor["transform"].as_i64().unwrap_or(0) % 2 == 1 {
            std::mem::swap(&mut width, &mut height);
        }
        let (left, top) = (monitor["x"].as_f64()?, monitor["y"].as_f64()?);
        let inside = x >= left && x < left + width && y >= top && y < top + height;
        inside.then(|| monitor["name"].as_str().map(str::to_owned))?
    })
}

/// The events worth passing on.
fn event(line: &str) -> Option<Event> {
    if let Some(output) = focused_from_event(line) {
        return Some(Event::Focus(output.to_owned()));
    }
    if let Some((started, target)) = captured_from_event(line) {
        return Some(Event::Captured {
            started,
            target: target.to_owned(),
        });
    }
    screencast_from_event(line).map(Event::Screencast)
}

/// `screencastv2>>1,monitor,DP-3`: the `screencast` event again, with the
/// monitor captured, or the window. A region names its monitor.
fn captured_from_event(line: &str) -> Option<(bool, &str)> {
    let rest = line.strip_prefix("screencastv2>>")?;
    let mut parts = rest.splitn(3, ',');
    let started = match parts.next()? {
        "1" => true,
        "0" => false,
        _ => return None,
    };
    let _kind = parts.next()?;
    let target = parts.next().filter(|target| !target.is_empty())?;
    Some((started, target))
}

/// `screencast>>1,monitor`: a capture started (1) or stopped (0), of a
/// monitor or a window. Older Hyprland writes the owner as 0 or 1. The
/// `screencastv2` event repeats it with the output; one is enough.
fn screencast_from_event(line: &str) -> Option<bool> {
    let rest = line.strip_prefix("screencast>>")?;
    match rest.split_once(',').map_or(rest, |(state, _)| state) {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

/// `focusedmon>>DP-3,2` names the newly focused output.
fn focused_from_event(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("focusedmon>>")?;
    let (output, _workspace) = rest.split_once(',')?;
    (!output.is_empty()).then_some(output)
}

/// The output marked `"focused": true` in `hyprctl -j monitors` output.
fn focused_from_monitors(json: &str) -> Option<String> {
    let monitors: serde_json::Value = serde_json::from_str(json).ok()?;
    monitors
        .as_array()?
        .iter()
        .find(|monitor| monitor["focused"] == true)?["name"]
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_monitor_under_the_pointer() {
        let monitors = r#"[
            {"name": "HDMI-A-1", "x": 0, "y": 0, "width": 1920, "height": 1080, "scale": 1, "transform": 0},
            {"name": "DP-3", "x": 1920, "y": 0, "width": 3840, "height": 2160, "scale": 2, "transform": 0},
            {"name": "eDP-1", "x": 3840, "y": 0, "width": 1920, "height": 1080, "scale": 1, "transform": 1}
        ]"#;
        let at = |x: i32, y: i32| output_at(&format!(r#"{{"x": {x}, "y": {y}}}"#), monitors);
        assert_eq!(at(10, 10).as_deref(), Some("HDMI-A-1"));
        // Scaled: 3840 pixels at 2 cover 1920 in the layout.
        assert_eq!(at(3839, 1079).as_deref(), Some("DP-3"));
        // Rotated: 1080 wide, 1920 tall.
        assert_eq!(at(3900, 1500).as_deref(), Some("eDP-1"));
        assert_eq!(at(5000, 10), None);
    }

    #[test]
    fn reads_focus_events_only() {
        assert_eq!(
            focused_from_event("focusedmon>>HDMI-A-1,10"),
            Some("HDMI-A-1")
        );
        // The v2 event repeats it with the workspace id; one is enough.
        assert_eq!(focused_from_event("focusedmonv2>>HDMI-A-1,10"), None);
        assert_eq!(focused_from_event("workspace>>3"), None);
        assert_eq!(focused_from_event("focusedmon>>,3"), None);
    }

    #[test]
    fn reads_screencast_events() {
        assert_eq!(
            event("screencast>>1,monitor"),
            Some(Event::Screencast(true))
        );
        assert_eq!(
            event("screencast>>0,window"),
            Some(Event::Screencast(false))
        );
        assert_eq!(event("screencast>>1,0"), Some(Event::Screencast(true)));
        assert_eq!(
            event("screencastv2>>1,region,MOCHI-SHARE"),
            Some(Event::Captured {
                started: true,
                target: "MOCHI-SHARE".into()
            })
        );
        assert_eq!(event("screencastv2>>0,monitor,"), None);
        assert_eq!(event("screencast>>maybe"), None);
        assert_eq!(
            event("focusedmon>>DP-3,2"),
            Some(Event::Focus("DP-3".into()))
        );
        assert_eq!(event("openwindow>>abc"), None);
    }

    #[test]
    fn lists_visible_windows_topmost_first() {
        let monitors = r#"[
            {"name":"DP-3","activeWorkspace":{"id":2},"specialWorkspace":{"id":0}},
            {"name":"HDMI-A-1","activeWorkspace":{"id":10},"specialWorkspace":{"id":0}}
        ]"#;
        let clients = r#"[
            {"mapped":true,"hidden":false,"at":[1942,62],"size":[1876,996],
             "workspace":{"id":2},"floating":false,"fullscreen":0,
             "class":"codium","title":"Editor","focusHistoryID":1},
            {"mapped":true,"hidden":false,"at":[2100,200],"size":[600,400],
             "workspace":{"id":2},"floating":true,"fullscreen":0,
             "class":"pavucontrol","title":"Volume","focusHistoryID":3},
            {"mapped":true,"hidden":false,"at":[0,0],"size":[1920,1080],
             "workspace":{"id":3},"floating":false,"fullscreen":0,
             "class":"firefox","title":"Hidden away","focusHistoryID":0},
            {"mapped":true,"hidden":false,"at":[10,62],"size":[931,996],
             "workspace":{"id":10},"floating":false,"fullscreen":0,
             "class":"spotify","title":"Music","focusHistoryID":2}
        ]"#;
        let windows = windows_from(clients, monitors).unwrap();
        let titles: Vec<&str> = windows.iter().map(|window| window.title.as_str()).collect();
        assert_eq!(titles, ["Volume", "Editor", "Music"]);
        assert_eq!(
            (
                windows[0].x,
                windows[0].y,
                windows[0].width,
                windows[0].height
            ),
            (2100, 200, 600, 400)
        );
        assert_eq!(windows[0].app_id, "pavucontrol");
    }

    #[test]
    fn trusts_the_visible_flag_when_present() {
        let monitors = r#"[{"activeWorkspace":{"id":1},"specialWorkspace":{"id":0}}]"#;
        let clients = r#"[
            {"at":[0,0],"size":[10,10],"workspace":{"id":1},"visible":false,"title":"a"},
            {"at":[0,0],"size":[10,10],"workspace":{"id":5},"visible":true,"title":"b"}
        ]"#;
        let windows = windows_from(clients, monitors).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].title, "b");
        assert_eq!(windows_from("nope", monitors), None);
    }

    #[test]
    fn finds_the_focused_monitor() {
        let json = r#"[{"name":"HDMI-A-1","focused":false},{"name":"DP-3","focused":true}]"#;
        assert_eq!(focused_from_monitors(json).as_deref(), Some("DP-3"));
        assert_eq!(focused_from_monitors("[]"), None);
        assert_eq!(focused_from_monitors("not json"), None);
    }
}

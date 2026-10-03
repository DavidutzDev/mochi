//! Hyprland's IPC, used only for what the standard protocols don't say: which
//! output has focus. Focusing an empty workspace on another monitor moves no
//! window, so the standard window-based guess can't see it; Hyprland's event
//! socket can.
//!
//! The focused output name goes to the Wayland task, which owns the model.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::UnboundedSender;

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(30);

/// Hyprland's socket directory, when running under Hyprland.
pub(crate) fn socket_dir() -> Option<PathBuf> {
    let instance = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let dir = Path::new(&runtime).join("hypr").join(instance);
    dir.join(".socket2.sock").exists().then_some(dir)
}

/// Sends the focused output now, then every time it changes. Reconnects with
/// backoff if the socket closes. Ends when nobody listens any more.
pub(crate) async fn watch_focus(dir: PathBuf, focus: UnboundedSender<String>) {
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
async fn session(dir: &Path, focus: &UnboundedSender<String>) -> std::io::Result<()> {
    // Connect to the events first, so no change slips in between the query
    // and the subscription.
    let events = UnixStream::connect(dir.join(".socket2.sock")).await?;
    if let Some(output) = query_focused(dir).await?
        && focus.send(output).is_err()
    {
        return Ok(());
    }

    let mut lines = BufReader::new(events).lines();
    while let Some(line) = lines.next_line().await? {
        if let Some(output) = focused_from_event(&line)
            && focus.send(output.to_owned()).is_err()
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
    let mut request = UnixStream::connect(dir.join(".socket.sock")).await?;
    request.write_all(b"j/monitors").await?;
    let mut reply = String::new();
    request.read_to_string(&mut reply).await?;
    Ok(focused_from_monitors(&reply))
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
    fn finds_the_focused_monitor() {
        let json = r#"[{"name":"HDMI-A-1","focused":false},{"name":"DP-3","focused":true}]"#;
        assert_eq!(focused_from_monitors(json).as_deref(), Some("DP-3"));
        assert_eq!(focused_from_monitors("[]"), None);
        assert_eq!(focused_from_monitors("not json"), None);
    }
}

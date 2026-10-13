//! Which monitors' desktops windows hide, for the desktop widgets: what
//! only decorates stops moving there, as under a fullscreen window. Asks
//! the compositor's IPC where the windows are when a window opens, closes
//! or takes focus, or the workspaces change, and every few seconds for one
//! moved without any of that. Only while the widgets module runs.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mochi_core::compositor::{Compositor, CompositorError};
use tokio::sync::mpsc::UnboundedSender;

/// How much of a monitor its windows must hide: a tiled window with gaps
/// and the island's strip hides about 90%.
const SHARE: f64 = 0.85;
/// How long to let a burst of changes, like a workspace switch, settle.
const SETTLE: Duration = Duration::from_millis(150);
/// How often to look anyway, for a floating window moved.
const POLL: Duration = Duration::from_secs(3);

/// Sends the covered desktops whenever they change. `wanted` says whether
/// anyone draws widgets: without, it sends an empty list once and waits.
pub fn spawn(
    compositor: Compositor,
    wanted: Arc<AtomicBool>,
    sender: UnboundedSender<Vec<String>>,
) {
    if !compositor.knows_windows() {
        return;
    }
    tokio::spawn(async move {
        let mut state = compositor.subscribe();
        let mut windows = compositor.subscribe_toplevels();
        let mut last: Option<Vec<String>> = None;
        // The first look is at once, for windows open before Mochi started.
        let mut first = true;
        loop {
            if !std::mem::take(&mut first) {
                tokio::select! {
                    changed = state.changed() => if changed.is_err() { return },
                    changed = windows.changed() => if changed.is_err() { return },
                    () = tokio::time::sleep(POLL) => {}
                }
                tokio::time::sleep(SETTLE).await;
            }
            let covered = if wanted.load(Ordering::Relaxed) {
                match compositor.desktops_covered(SHARE).await {
                    Ok(covered) => covered,
                    Err(CompositorError::NoWindowGeometry) => return,
                    Err(error) => {
                        tracing::debug!(%error, "can't tell where the windows are");
                        continue;
                    }
                }
            } else {
                Vec::new()
            };
            if last.as_ref() != Some(&covered) {
                if sender.send(covered.clone()).is_err() {
                    return;
                }
                last = Some(covered);
            }
        }
    });
}

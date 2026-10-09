//! The keyboard layout, from the compositor's state. No standard protocol
//! says it; Hyprland, niri and Sway do through their IPC, and elsewhere it
//! stays unknown.

use mochi_core::compositor::Compositor;
use tokio::sync::mpsc::UnboundedSender;

use crate::notice::Change;

/// Reports the layout now, then each time it changes.
pub async fn watch(compositor: Compositor, changes: UnboundedSender<Change>) {
    let mut updates = compositor.subscribe();
    let mut last = None;
    loop {
        let layout = updates.borrow_and_update().keyboard_layout.clone();
        if last.as_ref() != Some(&layout) {
            if changes.send(Change::Layout(layout.clone())).is_err() {
                return;
            }
            last = Some(layout);
        }
        if updates.changed().await.is_err() {
            return;
        }
    }
}

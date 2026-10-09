//! Compositor IPC, for what the standard protocols don't say: which output
//! has focus, where windows are, what's being shared, and the keyboard
//! layout. Hyprland, niri and Sway each have their own; on any other
//! compositor, the protocols are all there is.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

use crate::{Window, hyprland, niri, sway};

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(30);

/// What the IPC tells the Wayland task, which owns the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// The output with focus.
    Focus(String),
    /// A screencast started (true) or stopped. Hyprland's.
    Screencast(bool),
    /// The same, with what it captures: a monitor's name, or a window's.
    Captured { started: bool, target: String },
    /// Every capture running now, by what it captures: niri's, which says
    /// them all at once.
    Casts(Vec<String>),
    /// The active keyboard layout's name, like `English (US)`.
    KeyboardLayout(String),
    /// The IPC (re)connected: counts kept so far may be stale.
    Connected,
}

/// The session compositor's IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Ipc {
    /// Its socket directory.
    Hyprland(PathBuf),
    /// `$NIRI_SOCKET`.
    Niri(PathBuf),
    /// `$SWAYSOCK`.
    Sway(PathBuf),
}

impl Ipc {
    /// The IPC of the compositor running, when Mochi knows it.
    pub(crate) fn find() -> Option<Self> {
        if let Some(dir) = hyprland::socket_dir() {
            return Some(Self::Hyprland(dir));
        }
        let socket = |name: &str| {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .filter(|path| path.exists())
        };
        socket("NIRI_SOCKET")
            .map(Self::Niri)
            .or_else(|| socket("SWAYSOCK").map(Self::Sway))
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Hyprland(_) => "Hyprland",
            Self::Niri(_) => "niri",
            Self::Sway(_) => "Sway",
        }
    }

    /// Hyprland's socket directory, for what only it can do.
    pub(crate) fn hyprland(&self) -> Option<&Path> {
        match self {
            Self::Hyprland(dir) => Some(dir),
            _ => None,
        }
    }

    /// Sends the focused output now, then every change it reports.
    /// Reconnects with backoff when the socket closes. Ends when nobody
    /// listens any more.
    pub(crate) async fn watch(self, events: UnboundedSender<Event>) {
        let mut retry = FIRST_RETRY;
        loop {
            let session = match &self {
                Self::Hyprland(dir) => hyprland::session(dir, &events).await,
                Self::Niri(socket) => niri::session(socket, &events).await,
                Self::Sway(socket) => sway::session(socket, &events).await,
            };
            match session {
                Ok(()) => return,
                Err(error) => {
                    tracing::warn!(%error, retry = ?retry, ipc = self.name(), "lost the compositor's IPC");
                    tokio::time::sleep(retry).await;
                    retry = (retry * 2).min(MAX_RETRY);
                }
            }
        }
    }

    /// The windows on visible workspaces, topmost first.
    pub(crate) async fn windows(&self) -> std::io::Result<Vec<Window>> {
        match self {
            Self::Hyprland(dir) => hyprland::windows(dir).await,
            Self::Niri(socket) => niri::windows(socket).await,
            Self::Sway(socket) => sway::windows(socket).await,
        }
    }
}

/// A keyboard layout's name worth showing. Keymaps that apps make to type
/// through a virtual keyboard, Mochi's included, often name no layout, and
/// Hyprland then says `error` or `none`.
pub(crate) fn layout_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty() && name != "error" && name != "none").then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unnamed_layouts_are_left_out() {
        assert_eq!(layout_name("English (UK)").as_deref(), Some("English (UK)"));
        assert_eq!(layout_name("error"), None);
        assert_eq!(layout_name("none"), None);
        assert_eq!(layout_name(" "), None);
    }
}

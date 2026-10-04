//! What modules know about the compositor, independent of which compositor
//! it is.
//!
//! The state comes from standard Wayland protocols, so any compositor that
//! supports them works without compositor-specific code: workspaces from
//! `ext-workspace-v1`, output names from `wl_output`, and the focused output
//! from the focused window (`wlr-foreign-toplevel-management`). Without a
//! Wayland session, or on a compositor without the protocols, [`connect`]
//! returns a handle whose state says [`Backend::Unsupported`], and modules
//! keep working.
//!
//! Compositor IPC only fills gaps the standards leave, and modules never see
//! it: on Hyprland, its event socket reports the focused output exactly,
//! including when focus moves to an empty workspace.
//!
//! Modules read the latest [`State`], wait for changes with
//! [`Compositor::subscribe`], and act with methods like
//! [`Compositor::activate_workspace`].

mod hyprland;
mod model;
mod wayland;

use std::fmt;

use tokio::sync::{mpsc, watch};

/// Where the state comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    /// No compositor information is available.
    #[default]
    Unsupported,
    /// Standard Wayland protocols.
    Wayland,
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unsupported => "unsupported",
            Self::Wayland => "wayland",
        })
    }
}

/// A snapshot of the compositor, replaced as a whole on every change.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub backend: Backend,
    /// Sorted by name.
    pub outputs: Vec<Output>,
    /// Grouped by output, each output's workspaces in display order.
    pub workspaces: Vec<Workspace>,
    /// The output with keyboard focus, by name, when the compositor says.
    pub focused_output: Option<String>,
    /// Something is capturing the screen, when the compositor says: Hyprland
    /// does, through its event socket. That includes any capture, even a
    /// one-frame screenshot or a live thumbnail, so a module that shows it
    /// should wait a moment before believing it.
    pub screencast: bool,
}

impl State {
    /// The active workspace on an output.
    pub fn active_on(&self, output: &str) -> Option<&Workspace> {
        self.workspaces
            .iter()
            .find(|workspace| workspace.active && workspace.output.as_deref() == Some(output))
    }

    pub fn workspaces_on<'a>(&'a self, output: &'a str) -> impl Iterator<Item = &'a Workspace> {
        self.workspaces
            .iter()
            .filter(move |workspace| workspace.output.as_deref() == Some(output))
    }

    pub fn workspace(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|workspace| workspace.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The connector name, such as `DP-3`.
    pub name: String,
    /// A human-readable name, often the monitor model.
    pub description: String,
}

/// Valid while the workspace exists. Compositors may reuse it afterwards, so
/// to recognize a workspace across changes, compare its output and name.
/// Public so modules can build states in their tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkspaceId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    /// The output it is on, by name.
    pub output: Option<String>,
    /// Its position in the compositor's layout, when it has one.
    pub coordinates: Vec<u32>,
    pub active: bool,
    pub urgent: bool,
    pub hidden: bool,
    /// Whether the compositor lets clients switch to it.
    pub can_activate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompositorError {
    #[error("no compositor information is available")]
    Unsupported,
    #[error("workspace {0:?} does not exist")]
    UnknownWorkspace(WorkspaceId),
    #[error("the compositor doesn't allow switching to workspace {0:?}")]
    NotAllowed(WorkspaceId),
    #[error("this compositor doesn't say where its windows are")]
    NoWindowGeometry,
    #[error("the compositor's IPC failed: {0}")]
    Ipc(String),
}

/// A window on a visible workspace, for picking one on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub title: String,
    pub app_id: String,
    /// Position and size in the compositor's global layout, in logical
    /// pixels: the same space as the outputs' positions.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub floating: bool,
}

/// Wakes on every new snapshot; see [`Compositor::subscribe`].
pub type StateReceiver = watch::Receiver<State>;

#[derive(Debug)]
pub(crate) enum Action {
    ActivateWorkspace(WorkspaceId),
}

/// A cheap, cloneable handle to the compositor.
#[derive(Debug, Clone)]
pub struct Compositor {
    state: watch::Receiver<State>,
    actions: mpsc::UnboundedSender<Action>,
    /// Hyprland's socket directory, under Hyprland.
    hyprland: Option<std::path::PathBuf>,
}

impl Compositor {
    /// A handle with no compositor behind it.
    pub fn unsupported() -> Self {
        let (_, state) = watch::channel(State::default());
        let (actions, _) = mpsc::unbounded_channel();
        Self {
            state,
            actions,
            hyprland: None,
        }
    }

    /// The latest snapshot.
    pub fn state(&self) -> State {
        self.state.borrow().clone()
    }

    /// A receiver that wakes on every new snapshot. Its `changed()` fails
    /// once no more snapshots will come.
    pub fn subscribe(&self) -> StateReceiver {
        self.state.clone()
    }

    /// Switches to a workspace on its output.
    pub fn activate_workspace(&self, id: WorkspaceId) -> Result<(), CompositorError> {
        let state = self.state.borrow();
        if state.backend == Backend::Unsupported {
            return Err(CompositorError::Unsupported);
        }
        let workspace = state
            .workspace(id)
            .ok_or(CompositorError::UnknownWorkspace(id))?;
        if !workspace.can_activate {
            return Err(CompositorError::NotAllowed(id));
        }
        self.actions
            .send(Action::ActivateWorkspace(id))
            .map_err(|_| CompositorError::Unsupported)
    }
}

impl Compositor {
    /// The windows on visible workspaces, topmost first. No standard
    /// protocol says where windows are, so this needs compositor IPC:
    /// Hyprland's for now. Elsewhere it fails with
    /// [`CompositorError::NoWindowGeometry`].
    pub async fn windows(&self) -> Result<Vec<Window>, CompositorError> {
        let dir = self
            .hyprland
            .as_deref()
            .ok_or(CompositorError::NoWindowGeometry)?;
        hyprland::windows(dir)
            .await
            .map_err(|error| CompositorError::Ipc(error.to_string()))
    }

    /// Whether [`Compositor::windows`] can work here.
    pub fn knows_windows(&self) -> bool {
        self.hyprland.is_some()
    }
}

/// Connects to the compositor of the current session. Must run inside a
/// tokio runtime. Never fails: without a usable compositor the handle reports
/// [`Backend::Unsupported`].
pub fn connect() -> Compositor {
    match wayland::start() {
        Ok(compositor) => compositor,
        Err(reason) => {
            tracing::warn!(%reason, "no compositor information, modules that need it stay idle");
            Compositor::unsupported()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unsupported_handle_refuses_actions() {
        let compositor = Compositor::unsupported();
        assert_eq!(compositor.state().backend, Backend::Unsupported);
        assert_eq!(
            compositor.activate_workspace(WorkspaceId(1)),
            Err(CompositorError::Unsupported)
        );
    }

    #[test]
    fn actions_are_checked_against_the_state() {
        let workspace = |id, can_activate| Workspace {
            id: WorkspaceId(id),
            name: id.to_string(),
            output: Some("DP-3".into()),
            coordinates: Vec::new(),
            active: false,
            urgent: false,
            hidden: false,
            can_activate,
        };
        let (sender, state) = watch::channel(State {
            backend: Backend::Wayland,
            outputs: Vec::new(),
            workspaces: vec![workspace(1, true), workspace(2, false)],
            focused_output: None,
            screencast: false,
        });
        let (actions, mut received) = mpsc::unbounded_channel();
        let compositor = Compositor {
            state,
            actions,
            hyprland: None,
        };

        assert_eq!(compositor.activate_workspace(WorkspaceId(1)), Ok(()));
        assert!(matches!(
            received.try_recv(),
            Ok(Action::ActivateWorkspace(WorkspaceId(1)))
        ));
        assert_eq!(
            compositor.activate_workspace(WorkspaceId(2)),
            Err(CompositorError::NotAllowed(WorkspaceId(2)))
        );
        assert_eq!(
            compositor.activate_workspace(WorkspaceId(9)),
            Err(CompositorError::UnknownWorkspace(WorkspaceId(9)))
        );
        drop(sender);
    }
}

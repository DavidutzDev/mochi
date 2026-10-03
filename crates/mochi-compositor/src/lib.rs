//! What modules know about the compositor, independent of which compositor
//! it is.
//!
//! The state comes from standard Wayland protocols, so any compositor that
//! supports them works without compositor-specific code: workspaces from
//! `ext-workspace-v1` and output names from `wl_output`. Without a Wayland
//! session, or on a compositor without the protocols, [`connect`] returns a
//! handle whose state says [`Backend::Unsupported`], and modules keep working.
//!
//! Modules read the latest [`State`], wait for changes with
//! [`Compositor::subscribe`], and act with methods like
//! [`Compositor::activate_workspace`].

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

/// Valid while the workspace exists. Compositors may reuse it afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkspaceId(pub(crate) u32);

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
}

#[derive(Debug)]
pub(crate) enum Action {
    ActivateWorkspace(WorkspaceId),
}

/// A cheap, cloneable handle to the compositor.
#[derive(Debug, Clone)]
pub struct Compositor {
    state: watch::Receiver<State>,
    actions: mpsc::UnboundedSender<Action>,
}

impl Compositor {
    /// A handle with no compositor behind it.
    pub fn unsupported() -> Self {
        let (_, state) = watch::channel(State::default());
        let (actions, _) = mpsc::unbounded_channel();
        Self { state, actions }
    }

    /// The latest snapshot.
    pub fn state(&self) -> State {
        self.state.borrow().clone()
    }

    /// A receiver that wakes on every new snapshot.
    pub fn subscribe(&self) -> watch::Receiver<State> {
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
        });
        let (actions, mut received) = mpsc::unbounded_channel();
        let compositor = Compositor { state, actions };

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

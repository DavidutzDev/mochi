//! The Wayland backend: a client connection that binds `ext-workspace-v1`
//! and every `wl_output`, and feeds their events into the [`Model`].
//!
//! The connection runs as a tokio task. It waits for the socket to become
//! readable or for an action from a module, whichever comes first, and
//! publishes a new snapshot each time the compositor marks a batch of
//! changes done.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::os::fd::{AsRawFd, RawFd};

use tokio::io::unix::AsyncFd;
use tokio::sync::{mpsc, watch};
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, event_created_child,
};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1},
    ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1},
    ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1},
};

use crate::model::Model;
use crate::{Action, Compositor, State};

/// `wl_output` version 4 added the `name` and `description` events.
const OUTPUT_VERSION: u32 = 4;

pub(crate) fn start() -> Result<Compositor, String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("cannot connect to the Wayland display: {error}"))?;
    let (globals, mut queue) = registry_queue_init::<Client>(&connection)
        .map_err(|error| format!("cannot read the Wayland globals: {error}"))?;
    let handle = queue.handle();

    let manager: ExtWorkspaceManagerV1 = globals
        .bind(&handle, 1..=1, ())
        .map_err(|_| "the compositor doesn't support ext-workspace-v1".to_owned())?;

    let mut client = Client {
        model: Model::default(),
        manager,
        workspaces: HashMap::new(),
        outputs: HashMap::new(),
        done: false,
        finished: false,
    };
    for global in globals.contents().clone_list() {
        if global.interface == wl_output::WlOutput::interface().name {
            client.bind_output(globals.registry(), &handle, global.name, global.version);
        }
    }

    // The first round trip delivers the workspaces and binds the outputs,
    // the second delivers the outputs' names.
    for _ in 0..2 {
        queue
            .roundtrip(&mut client)
            .map_err(|error| format!("Wayland round trip failed: {error}"))?;
    }

    let snapshot = client.model.snapshot();
    tracing::info!(
        outputs = snapshot.outputs.len(),
        workspaces = snapshot.workspaces.len(),
        "connected to the compositor through ext-workspace-v1"
    );
    let (state_sender, state) = watch::channel(snapshot);
    let (actions, action_receiver) = mpsc::unbounded_channel();
    tokio::spawn(run(
        connection,
        queue,
        client,
        state_sender,
        action_receiver,
    ));
    Ok(Compositor { state, actions })
}

async fn run(
    connection: Connection,
    mut queue: EventQueue<Client>,
    mut client: Client,
    state: watch::Sender<State>,
    mut actions: mpsc::UnboundedReceiver<Action>,
) {
    let socket = match AsyncFd::new(Socket(connection.backend().poll_fd().as_raw_fd())) {
        Ok(socket) => socket,
        Err(error) => {
            tracing::error!(%error, "cannot watch the Wayland socket");
            state.send_replace(State::default());
            return;
        }
    };

    loop {
        if let Err(error) = queue.dispatch_pending(&mut client) {
            tracing::error!(%error, "lost the Wayland connection");
            break;
        }
        if std::mem::take(&mut client.done) {
            let next = client.model.snapshot();
            state.send_if_modified(|current| {
                let changed = *current != next;
                *current = next;
                changed
            });
        }
        if client.finished {
            tracing::warn!("the compositor stopped sending workspace updates");
            break;
        }
        if let Err(error) = queue.flush() {
            tracing::error!(%error, "lost the Wayland connection");
            break;
        }

        let Some(guard) = queue.prepare_read() else {
            // Events are already queued; dispatch them first.
            continue;
        };
        tokio::select! {
            ready = socket.readable() => {
                let Ok(mut ready) = ready else { break };
                match guard.read() {
                    Ok(_) => {}
                    Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
                    Err(error) => {
                        tracing::error!(%error, "lost the Wayland connection");
                        break;
                    }
                }
                ready.clear_ready();
            }
            action = actions.recv() => {
                drop(guard);
                match action {
                    Some(action) => client.perform(action),
                    // Every handle is gone: nobody is listening any more.
                    None => return,
                }
            }
        }
    }

    // Tell modules there is nothing to rely on any more.
    state.send_replace(State::default());
}

/// The connection's file descriptor, for tokio to watch. The connection owns
/// it and outlives this wrapper.
struct Socket(RawFd);

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

struct Client {
    model: Model,
    manager: ExtWorkspaceManagerV1,
    /// Workspace handles by protocol id, to act on them.
    workspaces: HashMap<u32, ExtWorkspaceHandleV1>,
    /// Bound outputs by registry name, to release them when they go away.
    outputs: HashMap<u32, wl_output::WlOutput>,
    /// The compositor finished a batch of changes.
    done: bool,
    /// The compositor will send no more workspace events.
    finished: bool,
}

impl Client {
    fn bind_output(
        &mut self,
        registry: &wl_registry::WlRegistry,
        handle: &QueueHandle<Self>,
        name: u32,
        version: u32,
    ) {
        if version < OUTPUT_VERSION {
            tracing::warn!(version, "wl_output is too old to report output names");
            return;
        }
        let output = registry.bind::<wl_output::WlOutput, _, _>(name, OUTPUT_VERSION, handle, ());
        self.outputs.insert(name, output);
    }

    fn perform(&mut self, action: Action) {
        match action {
            Action::ActivateWorkspace(id) => match self.workspaces.get(&id.0) {
                Some(workspace) => {
                    workspace.activate();
                    self.manager.commit();
                }
                None => tracing::warn!(?id, "the workspace is gone"),
            },
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Client {
    fn event(
        client: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == wl_output::WlOutput::interface().name => {
                client.bind_output(registry, handle, name, version);
            }
            wl_registry::Event::GlobalRemove { name } => {
                if let Some(output) = client.outputs.remove(&name) {
                    client.model.output_removed(output.id().protocol_id());
                    output.release();
                    client.done = true;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Client {
    fn event(
        client: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = output.id().protocol_id();
        match event {
            wl_output::Event::Name { name } => client.model.output_name(id, name),
            wl_output::Event::Description { description } => {
                client.model.output_description(id, description);
            }
            wl_output::Event::Done => client.done = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for Client {
    fn event(
        client: &mut Self,
        _: &ExtWorkspaceManagerV1,
        event: ext_workspace_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_workspace_manager_v1::Event::WorkspaceGroup { workspace_group } => {
                client.model.group_added(workspace_group.id().protocol_id());
            }
            ext_workspace_manager_v1::Event::Workspace { workspace } => {
                let id = workspace.id().protocol_id();
                client.model.workspace_added(id);
                client.workspaces.insert(id, workspace);
            }
            ext_workspace_manager_v1::Event::Done => client.done = true,
            ext_workspace_manager_v1::Event::Finished => client.finished = true,
            _ => {}
        }
    }

    event_created_child!(Client, ExtWorkspaceManagerV1, [
        ext_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
        ext_workspace_manager_v1::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for Client {
    fn event(
        client: &mut Self,
        group: &ExtWorkspaceGroupHandleV1,
        event: ext_workspace_group_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = group.id().protocol_id();
        match event {
            ext_workspace_group_handle_v1::Event::OutputEnter { output } => {
                client
                    .model
                    .group_output(id, output.id().protocol_id(), true);
            }
            ext_workspace_group_handle_v1::Event::OutputLeave { output } => {
                client
                    .model
                    .group_output(id, output.id().protocol_id(), false);
            }
            ext_workspace_group_handle_v1::Event::WorkspaceEnter { workspace } => {
                client
                    .model
                    .group_workspace(id, workspace.id().protocol_id(), true);
            }
            ext_workspace_group_handle_v1::Event::WorkspaceLeave { workspace } => {
                client
                    .model
                    .group_workspace(id, workspace.id().protocol_id(), false);
            }
            ext_workspace_group_handle_v1::Event::Removed => {
                client.model.group_removed(id);
                group.destroy();
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for Client {
    fn event(
        client: &mut Self,
        workspace: &ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = workspace.id().protocol_id();
        match event {
            ext_workspace_handle_v1::Event::Name { name } => client.model.workspace_name(id, name),
            ext_workspace_handle_v1::Event::Coordinates { coordinates } => {
                // An array of native-endian u32 values.
                let coordinates = coordinates
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|bytes| u32::from_ne_bytes(*bytes))
                    .collect();
                client.model.workspace_coordinates(id, coordinates);
            }
            ext_workspace_handle_v1::Event::State { state } => {
                client
                    .model
                    .workspace_state(id, bits(state, |state| state.bits()));
            }
            ext_workspace_handle_v1::Event::Capabilities { capabilities } => {
                client.model.workspace_capabilities(
                    id,
                    bits(capabilities, |capabilities| capabilities.bits()),
                );
            }
            ext_workspace_handle_v1::Event::Removed => {
                client.model.workspace_removed(id);
                client.workspaces.remove(&id);
                workspace.destroy();
            }
            _ => {}
        }
    }
}

/// The raw bits of a bitfield, including ones this version doesn't know.
fn bits<T>(value: WEnum<T>, known: impl Fn(T) -> u32) -> u32 {
    match value {
        WEnum::Value(value) => known(value),
        WEnum::Unknown(raw) => raw,
    }
}

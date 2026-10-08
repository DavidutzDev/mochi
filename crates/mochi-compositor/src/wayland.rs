//! The Wayland backend: a client connection that binds `ext-workspace-v1`,
//! every `wl_output` and, when available, `wlr-foreign-toplevel-management`,
//! and feeds their events into the [`Model`].
//!
//! The connection runs as a tokio task. It waits for the socket to become
//! readable, for an action from a module, or for a focus report from
//! compositor IPC, and publishes a new snapshot each time the compositor
//! marks a batch of changes done.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::os::fd::{AsRawFd, RawFd};

use tokio::io::unix::AsyncFd;
use tokio::sync::{mpsc, watch};
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_output, wl_pointer, wl_registry, wl_seat};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, event_created_child,
};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1},
    ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1},
    ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1},
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

use crate::ipc::{Event, Ipc};
use crate::model::Model;
use crate::{Action, Compositor, State};

/// `wl_output` version 4 added the `name` and `description` events.
const OUTPUT_VERSION: u32 = 4;
/// `zwlr_foreign_toplevel_handle_v1.state` value for the focused window.
const ACTIVATED: u32 = 2;

/// Answers nothing: only the registry's list is read.
struct Probe;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Probe {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

pub(crate) fn globals() -> Result<Vec<String>, String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("cannot connect to the Wayland display: {error}"))?;
    let (globals, _queue) = registry_queue_init::<Probe>(&connection)
        .map_err(|error| format!("cannot read the Wayland globals: {error}"))?;
    let mut names: Vec<String> = globals
        .contents()
        .clone_list()
        .into_iter()
        .map(|global| global.interface)
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

pub(crate) fn start() -> Result<Compositor, String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("cannot connect to the Wayland display: {error}"))?;
    let (globals, mut queue) = registry_queue_init::<Client>(&connection)
        .map_err(|error| format!("cannot read the Wayland globals: {error}"))?;
    let handle = queue.handle();

    // Outputs first: the compositor only tells a client that a window is on
    // an output once the client has bound that output.
    let mut outputs = HashMap::new();
    for global in globals.contents().clone_list() {
        if global.interface == wl_output::WlOutput::interface().name {
            bind_output(
                &mut outputs,
                globals.registry(),
                &handle,
                global.name,
                global.version,
            );
        }
    }

    let manager: ExtWorkspaceManagerV1 = globals
        .bind(&handle, 1..=1, ())
        .map_err(|_| "the compositor doesn't support ext-workspace-v1".to_owned())?;
    // Windows only tell which output has focus, so they're optional.
    let toplevels: Option<ZwlrForeignToplevelManagerV1> = globals.bind(&handle, 1..=3, ()).ok();
    if toplevels.is_none() {
        tracing::info!(
            "no wlr-foreign-toplevel-management: the focused output comes from IPC only"
        );
    }

    // Activating a window names the seat whose focus it takes.
    let seat: Option<wl_seat::WlSeat> = globals.bind(&handle, 1..=1, ()).ok();
    // For clicks a layer surface caught that belong to the window under it.
    let pointers: Option<ZwlrVirtualPointerManagerV1> = globals.bind(&handle, 1..=2, ()).ok();

    let mut client = Client {
        model: Model::default(),
        manager,
        workspaces: HashMap::new(),
        outputs,
        handles: HashMap::new(),
        seat,
        pointers,
        pointer: None,
        started: std::time::Instant::now(),
        done: false,
        windows_changed: false,
        finished: false,
    };

    // The first round trip delivers the workspaces and binds the outputs,
    // the second delivers the outputs' names.
    for _ in 0..2 {
        queue
            .roundtrip(&mut client)
            .map_err(|error| format!("Wayland round trip failed: {error}"))?;
    }

    // Compositor IPC fills in what the protocols can't say.
    let ipc = Ipc::find();
    let focus = ipc.clone().map(|ipc| {
        let (sender, receiver) = mpsc::unbounded_channel();
        tokio::spawn(ipc.watch(sender));
        receiver
    });

    let snapshot = client.model.snapshot();
    tracing::info!(
        outputs = snapshot.outputs.len(),
        workspaces = snapshot.workspaces.len(),
        windows = toplevels.is_some(),
        ipc = ipc.as_ref().map(Ipc::name),
        "connected to the compositor through ext-workspace-v1"
    );
    let (state_sender, state) = watch::channel(snapshot);
    let (toplevel_sender, toplevels) = watch::channel(client.model.toplevels());
    let (actions, action_receiver) = mpsc::unbounded_channel();
    tokio::spawn(run(
        connection,
        queue,
        client,
        Senders {
            state: state_sender,
            toplevels: toplevel_sender,
        },
        action_receiver,
        focus,
    ));
    Ok(Compositor {
        state,
        actions,
        ipc,
        toplevels,
    })
}

/// Where the snapshots go.
struct Senders {
    state: watch::Sender<State>,
    toplevels: watch::Sender<Vec<crate::Toplevel>>,
}

async fn run(
    connection: Connection,
    mut queue: EventQueue<Client>,
    mut client: Client,
    senders: Senders,
    mut actions: mpsc::UnboundedReceiver<Action>,
    mut focus: Option<mpsc::UnboundedReceiver<Event>>,
) {
    let socket = match AsyncFd::new(Socket(connection.backend().poll_fd().as_raw_fd())) {
        Ok(socket) => socket,
        Err(error) => {
            tracing::error!(%error, "cannot watch the Wayland socket");
            senders.state.send_replace(State::default());
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
            senders.state.send_if_modified(|current| {
                let changed = *current != next;
                *current = next;
                changed
            });
        }
        if std::mem::take(&mut client.windows_changed) {
            let next = client.model.toplevels();
            senders.toplevels.send_if_modified(|current| {
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
                    Some(action) => client.perform(action, &queue.handle()),
                    // Every handle is gone: nobody is listening any more.
                    None => return,
                }
            }
            event = next_ipc(&mut focus) => {
                drop(guard);
                match event {
                    Some(Event::Focus(output)) => {
                        client.model.ipc_focus(output);
                        client.done = true;
                    }
                    Some(Event::Screencast(active)) => {
                        client.model.ipc_screencast(active);
                        client.done = true;
                    }
                    Some(Event::Captured { started, target }) => {
                        client.model.ipc_captured(target, started);
                        client.done = true;
                    }
                    Some(Event::Casts(targets)) => {
                        client.model.ipc_casts(&targets);
                        client.done = true;
                    }
                    Some(Event::Connected) => {
                        client.model.ipc_connected();
                        client.done = true;
                    }
                    None => focus = None,
                }
            }
        }
    }

    // Tell modules there is nothing to rely on any more.
    senders.state.send_replace(State::default());
    senders.toplevels.send_replace(Vec::new());
}

/// The next report from IPC. Never returns when there is no IPC.
async fn next_ipc(focus: &mut Option<mpsc::UnboundedReceiver<Event>>) -> Option<Event> {
    match focus {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
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
    /// Window handles by protocol id, to activate them.
    handles: HashMap<u32, ZwlrForeignToplevelHandleV1>,
    seat: Option<wl_seat::WlSeat>,
    pointers: Option<ZwlrVirtualPointerManagerV1>,
    /// The virtual pointer, made once and kept: one made per click and
    /// dropped at once can lose its events.
    pointer: Option<(u32, ZwlrVirtualPointerV1)>,
    /// Events carry milliseconds from here.
    started: std::time::Instant,
    /// The compositor finished a batch of changes.
    done: bool,
    /// A window opened, closed or changed its title, app or focus.
    windows_changed: bool,
    /// The compositor will send no more workspace events.
    finished: bool,
}

/// Binds an output, keyed by its registry name so it can be released when it
/// goes away.
fn bind_output(
    outputs: &mut HashMap<u32, wl_output::WlOutput>,
    registry: &wl_registry::WlRegistry,
    handle: &QueueHandle<Client>,
    name: u32,
    version: u32,
) {
    if version < OUTPUT_VERSION {
        tracing::warn!(version, "wl_output is too old to report output names");
        return;
    }
    let output = registry.bind::<wl_output::WlOutput, _, _>(name, OUTPUT_VERSION, handle, ());
    outputs.insert(name, output);
}

impl Client {
    fn perform(&mut self, action: Action, handle: &QueueHandle<Self>) {
        match action {
            Action::ActivateWorkspace(id) => match self.workspaces.get(&id.0) {
                Some(workspace) => {
                    workspace.activate();
                    self.manager.commit();
                }
                None => tracing::warn!(?id, "the workspace is gone"),
            },
            Action::AssumeCaptures(captured) => {
                self.model.assume_captures(&captured);
                self.done = true;
            }
            Action::Click { output, click } => self.click(&output, click, handle),
            Action::ActivateToplevel(id) => match (self.handles.get(&id), &self.seat) {
                (Some(handle), Some(seat)) => handle.activate(seat),
                (None, _) => tracing::warn!(id, "the window is gone"),
                (_, None) => tracing::warn!("no seat to activate a window with"),
            },
        }
    }
}

impl Client {
    /// Clicks through a virtual pointer tied to `output`, so its
    /// coordinates are that output's.
    fn click(&mut self, output: &str, click: crate::Click, handle: &QueueHandle<Self>) {
        let (Some(pointers), Some(seat)) = (&self.pointers, &self.seat) else {
            tracing::debug!("no virtual pointer to pass a click on with");
            return;
        };
        if pointers.version() < 2 {
            tracing::debug!("the virtual pointer can't be tied to an output");
            return;
        }
        let Some(id) = self.model.output_id(output) else {
            tracing::debug!(output, "no such output to click on");
            return;
        };
        let Some(wl_output) = self
            .outputs
            .values()
            .find(|candidate| candidate.id().protocol_id() == id)
        else {
            return;
        };
        // A pointer is tied to one output; another output gets a new one.
        if self.pointer.as_ref().is_none_or(|(on, _)| *on != id) {
            if let Some((_, old)) = self.pointer.take() {
                old.destroy();
            }
            let pointer = pointers.create_virtual_pointer_with_output(
                Some(seat),
                Some(wl_output),
                handle,
                (),
            );
            self.pointer = Some((id, pointer));
        }
        let Some((_, pointer)) = &self.pointer else {
            return;
        };
        let time = u32::try_from(self.started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let extent = |size: f64| size.round().max(1.0) as u32;
        pointer.motion_absolute(
            time,
            click.x.round() as u32,
            click.y.round() as u32,
            extent(click.width),
            extent(click.height),
        );
        pointer.frame();
        pointer.button(time, click.button, wl_pointer::ButtonState::Pressed);
        pointer.frame();
        pointer.button(time + 1, click.button, wl_pointer::ButtonState::Released);
        pointer.frame();
        tracing::debug!(output, x = click.x, y = click.y, "passed a click on");
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
                bind_output(&mut client.outputs, registry, handle, name, version);
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
            wl_output::Event::Mode {
                flags: WEnum::Value(flags),
                width,
                height,
                ..
            } if flags.contains(wl_output::Mode::Current) => {
                let size = |value: i32| u32::try_from(value).unwrap_or_default();
                client.model.output_mode(id, size(width), size(height));
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

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for Client {
    fn event(
        _: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        _: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // New windows arrive as handles, whose own events carry the state.
    }

    event_created_child!(Client, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for Client {
    fn event(
        client: &mut Self,
        toplevel: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = toplevel.id().protocol_id();
        client.handles.entry(id).or_insert_with(|| toplevel.clone());
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                client.model.toplevel_title(id, title);
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputEnter { output } => {
                client
                    .model
                    .toplevel_output(id, output.id().protocol_id(), true);
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputLeave { output } => {
                client
                    .model
                    .toplevel_output(id, output.id().protocol_id(), false);
            }
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                client.model.toplevel_app_id(id, app_id);
            }
            zwlr_foreign_toplevel_handle_v1::Event::State { state } => {
                // An array of native-endian u32 values.
                let activated = state
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|bytes| u32::from_ne_bytes(*bytes) == ACTIVATED);
                client.model.toplevel_activated(id, activated);
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                client.model.toplevel_done(id);
                client.done = true;
                client.windows_changed = true;
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                client.model.toplevel_closed(id);
                client.handles.remove(&id);
                client.windows_changed = true;
                toplevel.destroy();
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

impl Dispatch<wl_seat::WlSeat, ()> for Client {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for Client {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerManagerV1,
        _: <ZwlrVirtualPointerManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for Client {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerV1,
        _: <ZwlrVirtualPointerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

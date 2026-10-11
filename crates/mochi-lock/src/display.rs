//! The private display the lock screen's renderer draws on: a minimal
//! Wayland server that only the renderer can connect to.
//!
//! It has what Quickshell needs to draw a panel on each monitor and
//! nothing more: surfaces, shared memory, the layer shell, an output for
//! each real monitor at its size and scale, and a seat with a pointer. No
//! keyboard: what's typed stays in the lock. Whatever a client draws only
//! becomes pixels the lock copies onto its own surfaces, so a client here
//! can't unlock, can't see a key, and can't reach the real display.

use std::collections::HashMap;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use smithay::input::pointer::{ButtonEvent, MotionEvent, PointerHandle};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_server::backend::{
    ClientData, ClientId, DisconnectReason, GlobalId,
};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Display, DisplayHandle, ListeningSocket};
use smithay::utils::{SERIAL_COUNTER, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    BufferAssignment, CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes,
    with_states,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState,
};
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shm::{ShmHandler, ShmState, with_buffer_contents};
use smithay::{
    delegate_compositor, delegate_layer_shell, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
};

/// The pixels a client last drew for a monitor, as ARGB8888 bytes.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub at: Instant,
}

/// A real monitor, mirrored.
struct Screen {
    output: Output,
    global: GlobalId,
    /// In logical pixels, and its scale.
    size: (u32, u32),
    scale: i32,
}

/// The server's state: what smithay's handlers work on.
pub struct Server {
    handle: DisplayHandle,
    compositor: CompositorState,
    shm: ShmState,
    seats: SeatState<Server>,
    layers: WlrLayerShellState,
    /// Qt won't start without it; its windows are never shown.
    xdg: XdgShellState,
    pointer: PointerHandle<Server>,
    /// Kept for the tests, which check it has no keyboard.
    #[cfg_attr(not(test), allow(dead_code))]
    seat: Seat<Server>,
    screens: HashMap<String, Screen>,
    /// Each panel, with the monitor it's on.
    panels: Vec<(LayerSurface, String)>,
    /// The process allowed to connect: the renderer.
    allowed: Option<u32>,
    /// What each monitor's panel drew last.
    pub frames: HashMap<String, Frame>,
    /// Frame callbacks waiting for the next tick, so a client draws at a
    /// steady pace, not as fast as it can.
    callbacks: Vec<smithay::reexports::wayland_server::protocol::wl_callback::WlCallback>,
    started: Instant,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Server")
            .field("screens", &self.screens.keys().collect::<Vec<_>>())
            .field("panels", &self.panels.len())
            .finish_non_exhaustive()
    }
}

/// The display: the server's state, its socket, and smithay's display.
pub struct Private {
    pub server: Server,
    pub display: Display<Server>,
    pub socket: ListeningSocket,
}

impl std::fmt::Debug for Private {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.server.fmt(formatter)
    }
}

impl Private {
    /// A display listening at `path`.
    pub fn new(path: &Path) -> Result<Self, String> {
        let display: Display<Server> =
            Display::new().map_err(|error| format!("can't make a display: {error}"))?;
        let handle = display.handle();
        let socket = ListeningSocket::bind_absolute(path.to_owned())
            .map_err(|error| format!("can't listen at {}: {error}", path.display()))?;
        let mut seats = SeatState::new();
        let mut seat: Seat<Server> = seats.new_wl_seat(&handle, "mochi-lock");
        let pointer = seat.add_pointer();
        let server = Server {
            compositor: CompositorState::new::<Server>(&handle),
            shm: ShmState::new::<Server>(&handle, Vec::new()),
            layers: WlrLayerShellState::new::<Server>(&handle),
            xdg: XdgShellState::new::<Server>(&handle),
            handle,
            seats,
            pointer,
            seat,
            screens: HashMap::new(),
            panels: Vec::new(),
            allowed: None,
            frames: HashMap::new(),
            callbacks: Vec::new(),
            started: Instant::now(),
        };
        Ok(Self {
            server,
            display,
            socket,
        })
    }

    /// Takes the clients waiting at the socket, keeping only the renderer.
    /// Returns how many it kept.
    pub fn accept(&mut self) -> usize {
        let mut kept = 0;
        while let Ok(Some(stream)) = self.socket.accept() {
            if self.server.allowed.is_none() || peer(&stream) != self.server.allowed {
                // Anyone else hangs up at once.
                continue;
            }
            if self
                .server
                .handle
                .insert_client(stream, Arc::new(ClientState::default()))
                .is_ok()
            {
                kept += 1;
            }
        }
        kept
    }

    /// Handles what the clients sent, and sends them what's due.
    pub fn dispatch(&mut self) {
        let _ = self.display.dispatch_clients(&mut self.server);
        let _ = self.display.flush_clients();
    }
}

impl Server {
    /// Only this process may connect: the renderer, once started.
    pub fn allow(&mut self, pid: Option<u32>) {
        self.allowed = pid;
    }

    /// Mirrors a real monitor, or updates one.
    pub fn screen(&mut self, name: &str, size: (u32, u32), scale: i32) {
        let scale = scale.max(1);
        let mode = Mode {
            size: (size.0 as i32 * scale, size.1 as i32 * scale).into(),
            refresh: 60_000,
        };
        let screen = self.screens.entry(name.to_owned()).or_insert_with(|| {
            let output = Output::new(
                name.to_owned(),
                PhysicalProperties {
                    size: (0, 0).into(),
                    subpixel: Subpixel::Unknown,
                    make: "Mochi".into(),
                    model: name.to_owned(),
                },
            );
            let global = output.create_global::<Server>(&self.handle);
            Screen {
                output,
                global,
                size,
                scale,
            }
        });
        screen.size = size;
        screen.scale = scale;
        screen.output.change_current_state(
            Some(mode),
            Some(Transform::Normal),
            Some(Scale::Integer(scale)),
            Some((0, 0).into()),
        );
        screen.output.set_preferred(mode);
        let logical = (size.0 as i32, size.1 as i32);
        for (panel, on) in &self.panels {
            if on == name {
                panel.with_pending_state(|state| state.size = Some(logical.into()));
                panel.send_configure();
            }
        }
    }

    /// Forgets a monitor that went.
    pub fn forget(&mut self, name: &str) {
        if let Some(screen) = self.screens.remove(name) {
            self.handle.remove_global::<Server>(screen.global);
        }
        self.frames.remove(name);
    }

    /// The pointer over a monitor, at `(x, y)` in logical pixels.
    pub fn pointer_motion(&mut self, name: &str, x: f64, y: f64, time: u32) {
        let focus = self
            .panel_on(name)
            .map(|surface| (surface, (0.0, 0.0).into()));
        let event = MotionEvent {
            location: (x, y).into(),
            serial: SERIAL_COUNTER.next_serial(),
            time,
        };
        let pointer = self.pointer.clone();
        pointer.motion(self, focus, &event);
        pointer.frame(self);
    }

    pub fn pointer_button(&mut self, button: u32, pressed: bool, time: u32) {
        let event = ButtonEvent {
            serial: SERIAL_COUNTER.next_serial(),
            time,
            button,
            state: if pressed {
                smithay::backend::input::ButtonState::Pressed
            } else {
                smithay::backend::input::ButtonState::Released
            },
        };
        let pointer = self.pointer.clone();
        pointer.button(self, &event);
        pointer.frame(self);
    }

    /// The pointer left the lock's surfaces.
    pub fn pointer_left(&mut self, time: u32) {
        let event = MotionEvent {
            location: (0.0, 0.0).into(),
            serial: SERIAL_COUNTER.next_serial(),
            time,
        };
        let pointer = self.pointer.clone();
        pointer.motion(self, None, &event);
        pointer.frame(self);
    }

    fn panel_on(&self, name: &str) -> Option<WlSurface> {
        self.panels
            .iter()
            .find(|(_, on)| on == name)
            .map(|(panel, _)| panel.wl_surface().clone())
    }

    /// Lets the clients draw their next frame.
    pub fn tick(&mut self) {
        let time = self.started.elapsed().as_millis() as u32;
        for callback in self.callbacks.drain(..) {
            callback.done(time);
        }
    }

    /// The monitor a panel on `output` is for: that one, or else the first.
    fn monitor_of(&self, output: Option<&WlOutput>) -> Option<String> {
        output
            .and_then(Output::from_resource)
            .map(|output| output.name())
            .or_else(|| self.screens.keys().next().cloned())
    }
}

/// The process at the other end of a socket.
pub fn peer(stream: &UnixStream) -> Option<u32> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: getsockopt writes at most `length` bytes into `credentials`.
    let status = unsafe {
        libc::getsockopt(
            stream.as_fd().as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut credentials).cast(),
            &raw mut length,
        )
    };
    (status == 0)
        .then(|| u32::try_from(credentials.pid).ok())
        .flatten()
}

/// Copies a buffer's pixels out, as ARGB8888 with alpha forced opaque: the
/// lock screen never shows through.
fn copy(buffer: &WlBuffer) -> Option<Frame> {
    with_buffer_contents(buffer, |pointer, length, data| {
        let (width, height, stride) = (
            u32::try_from(data.width).ok()?,
            u32::try_from(data.height).ok()?,
            usize::try_from(data.stride).ok()?,
        );
        let offset = usize::try_from(data.offset).ok()?;
        let row = width as usize * 4;
        if !matches!(
            data.format,
            wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888
        ) || stride < row
            || offset + stride * height as usize > length
        {
            return None;
        }
        // SAFETY: smithay maps the pool for `length` bytes, and the bounds
        // were checked above.
        let pool = unsafe { std::slice::from_raw_parts(pointer, length) };
        let mut pixels = Vec::with_capacity(row * height as usize);
        for line in 0..height as usize {
            let start = offset + line * stride;
            pixels.extend_from_slice(&pool[start..start + row]);
        }
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
        Some(Frame {
            width,
            height,
            pixels,
            at: Instant::now(),
        })
    })
    .ok()
    .flatten()
}

impl CompositorHandler for Server {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .expect("every client here is inserted with ClientState")
            .compositor
    }

    fn commit(&mut self, surface: &WlSurface) {
        let monitor = self
            .panels
            .iter()
            .find(|(panel, _)| panel.wl_surface() == surface)
            .map(|(_, on)| on.clone());
        let (buffer, callbacks) = with_states(surface, |states| {
            let mut attributes = states.cached_state.get::<SurfaceAttributes>();
            let current = attributes.current();
            (
                current.buffer.take(),
                std::mem::take(&mut current.frame_callbacks),
            )
        });
        self.callbacks.extend(callbacks);
        if let Some(BufferAssignment::NewBuffer(buffer)) = buffer {
            if let (Some(monitor), Some(frame)) = (monitor, copy(&buffer)) {
                self.frames.insert(monitor, frame);
            }
            buffer.release();
        }
    }
}

impl BufferHandler for Server {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for Server {
    fn shm_state(&self) -> &ShmState {
        &self.shm
    }
}

impl OutputHandler for Server {}

impl SeatHandler for Server {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seats
    }
}

impl WlrLayerShellHandler for Server {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layers
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        _namespace: String,
    ) {
        let Some(monitor) = self.monitor_of(output.as_ref()) else {
            return;
        };
        if let Some(screen) = self.screens.get(&monitor) {
            let size = (screen.size.0 as i32, screen.size.1 as i32);
            surface.with_pending_state(|state| state.size = Some(size.into()));
        }
        surface.send_configure();
        self.panels.push((surface, monitor));
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        self.panels.retain(|(panel, _)| panel != &surface);
    }
}

#[derive(Default)]
struct ClientState {
    compositor: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}

delegate_compositor!(Server);
delegate_shm!(Server);
delegate_output!(Server);
delegate_seat!(Server);
delegate_layer_shell!(Server);
delegate_xdg_shell!(Server);

impl XdgShellHandler for Server {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg
    }

    // Windows and popups get an answer, so a client waiting for one goes
    // on, but never reach the screen: only panels do.
    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.send_configure();
    }

    fn new_popup(&mut self, surface: PopupSurface, _: PositionerState) {
        let _ = surface.send_configure();
    }

    fn grab(
        &mut self,
        _: PopupSurface,
        _: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        _: smithay::utils::Serial,
    ) {
    }

    fn reposition_request(&mut self, _: PopupSurface, _: PositionerState, _: u32) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_keyboard_and_only_the_renderer_connects() {
        let dir = std::env::temp_dir().join(format!("mochi-lock-display-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut private = Private::new(&dir.join("display")).unwrap();
        // Nothing drawn here can get a key: there is no keyboard to give.
        assert!(private.server.seat.get_keyboard().is_none());
        assert!(private.server.seat.get_pointer().is_some());

        // Nobody allowed: a connection hangs up.
        let first = UnixStream::connect(dir.join("display")).unwrap();
        assert_eq!(private.accept(), 0);
        // Another process allowed: this one still can't.
        private.server.allow(Some(std::process::id() + 1));
        let second = UnixStream::connect(dir.join("display")).unwrap();
        assert_eq!(private.accept(), 0);
        // This process allowed: it can.
        private.server.allow(Some(std::process::id()));
        let third = UnixStream::connect(dir.join("display")).unwrap();
        assert_eq!(private.accept(), 1);
        drop((first, second, third));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn mirrors_a_monitor() {
        let dir = std::env::temp_dir().join(format!("mochi-lock-screens-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut private = Private::new(&dir.join("display")).unwrap();
        private.server.screen("DP-3", (1920, 1080), 2);
        assert_eq!(private.server.screens["DP-3"].size, (1920, 1080));
        assert_eq!(private.server.screens["DP-3"].scale, 2);
        private.server.forget("DP-3");
        assert!(private.server.screens.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

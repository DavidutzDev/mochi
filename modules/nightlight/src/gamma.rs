//! The screens' gamma through `wlr-gamma-control-unstable-v1`, on a Wayland
//! connection of the module's own: a gamma control for every output while
//! the light is warm, set from [`crate::color::ramps`], and none at
//! daylight. Destroying a control gives the output its own gamma back, so
//! the screens return to normal when Mochi stops, even if it crashes.
//!
//! Outputs plugged in later get the same temperature. A compositor gives
//! one client each output's gamma: with another night light running, like
//! wlsunset, the control fails and the module says so.
//!
//! Like the clipboard's, it runs as a tokio task that waits for the socket
//! or for a new temperature.

use std::collections::HashMap;
use std::io::{ErrorKind, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, RawFd};

use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols_wlr::gamma_control::v1::client::{
    zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1,
    zwlr_gamma_control_v1::{self, ZwlrGammaControlV1},
};

use crate::color;

/// Sets the temperature: `None` gives the screens their own gamma back.
#[derive(Debug)]
pub struct Gamma {
    requests: mpsc::UnboundedSender<Option<u32>>,
}

impl Gamma {
    pub fn set(&self, kelvin: Option<u32>) -> Result<(), String> {
        self.requests
            .send(kelvin)
            .map_err(|_| "lost the night light's Wayland connection".to_owned())
    }
}

/// Connects, and sends a problem to `problems` when an output's gamma
/// can't be set.
pub fn start(problems: mpsc::UnboundedSender<String>) -> Result<Gamma, String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("cannot connect to the Wayland display: {error}"))?;
    let (globals, queue) = registry_queue_init::<Client>(&connection)
        .map_err(|error| format!("cannot read the Wayland globals: {error}"))?;
    let handle = queue.handle();
    let manager: ZwlrGammaControlManagerV1 = globals
        .bind(&handle, 1..=1, ())
        .map_err(|_| "the compositor doesn't support wlr-gamma-control-unstable-v1".to_owned())?;
    let mut client = Client {
        manager,
        outputs: HashMap::new(),
        kelvin: None,
        problems,
        lost: false,
    };
    globals.contents().with_list(|list| {
        for global in list {
            if global.interface == wl_output::WlOutput::interface().name {
                client.add_output(globals.registry(), global.name, global.version, &handle);
            }
        }
    });
    let (requests, receiver) = mpsc::unbounded_channel();
    tokio::spawn(run(connection, queue, client, receiver));
    Ok(Gamma { requests })
}

async fn run(
    connection: Connection,
    mut queue: EventQueue<Client>,
    mut client: Client,
    mut requests: mpsc::UnboundedReceiver<Option<u32>>,
) {
    let socket = match AsyncFd::new(Socket(connection.backend().poll_fd().as_raw_fd())) {
        Ok(socket) => socket,
        Err(error) => {
            tracing::error!(%error, "cannot watch the Wayland socket");
            return;
        }
    };
    let handle = queue.handle();
    loop {
        if let Err(error) = queue.dispatch_pending(&mut client) {
            tracing::error!(%error, "lost the night light's Wayland connection");
            return;
        }
        if let Err(error) = queue.flush() {
            tracing::error!(%error, "lost the night light's Wayland connection");
            return;
        }
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        tokio::select! {
            ready = socket.readable() => {
                let Ok(mut ready) = ready else { return };
                match guard.read() {
                    Ok(_) => {}
                    Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
                    Err(error) => {
                        tracing::error!(%error, "lost the night light's Wayland connection");
                        let _ = client.problems.send(error.to_string());
                        return;
                    }
                }
                ready.clear_ready();
            }
            request = requests.recv() => {
                drop(guard);
                match request {
                    Some(kelvin) => {
                        client.kelvin = kelvin;
                        client.lost = false;
                        client.apply_all(&handle);
                    }
                    // Dropping the controls with the connection restores
                    // the screens.
                    None => return,
                }
            }
        }
    }
}

/// The connection's file descriptor, for tokio to watch.
struct Socket(RawFd);

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

struct Output {
    output: wl_output::WlOutput,
    control: Option<ZwlrGammaControlV1>,
    /// Steps per channel, once the compositor says.
    size: Option<usize>,
}

struct Client {
    manager: ZwlrGammaControlManagerV1,
    /// By the output's global name.
    outputs: HashMap<u32, Output>,
    kelvin: Option<u32>,
    problems: mpsc::UnboundedSender<String>,
    /// A control failed since the last request: said once, not per output.
    lost: bool,
}

impl Client {
    fn add_output(
        &mut self,
        registry: &wl_registry::WlRegistry,
        name: u32,
        version: u32,
        handle: &QueueHandle<Self>,
    ) {
        let output = registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), handle, ());
        self.outputs.insert(
            name,
            Output {
                output,
                control: None,
                size: None,
            },
        );
        self.apply(name, handle);
    }

    fn apply_all(&mut self, handle: &QueueHandle<Self>) {
        let names: Vec<u32> = self.outputs.keys().copied().collect();
        for name in names {
            self.apply(name, handle);
        }
    }

    /// Brings one output to the temperature: a control and its table when
    /// warm, no control at daylight.
    fn apply(&mut self, name: u32, handle: &QueueHandle<Self>) {
        let Some(output) = self.outputs.get_mut(&name) else {
            return;
        };
        let Some(kelvin) = self.kelvin.filter(|kelvin| *kelvin < color::NEUTRAL) else {
            if let Some(control) = output.control.take() {
                control.destroy();
            }
            output.size = None;
            return;
        };
        let Some(control) = &output.control else {
            // The size comes back as an event, which applies the table.
            output.control = Some(self.manager.get_gamma_control(&output.output, handle, name));
            return;
        };
        let Some(size) = output.size else {
            return;
        };
        if let Err(error) = set_table(control, size, kelvin) {
            tracing::warn!(%error, "cannot set an output's gamma");
        }
    }
}

/// Hands the compositor the table in a memfd.
fn set_table(control: &ZwlrGammaControlV1, size: usize, kelvin: u32) -> std::io::Result<()> {
    let table = color::ramps(size, kelvin);
    let bytes: Vec<u8> = table.iter().flat_map(|step| step.to_ne_bytes()).collect();
    // SAFETY: the name is a valid C string, and memfd_create returns a new
    // descriptor or -1.
    let fd = unsafe { libc::memfd_create(c"mochi-gamma".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: memfd_create returned a new descriptor nobody else owns.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.write_all(&bytes)?;
    // Compositors read from where the file is.
    file.seek(SeekFrom::Start(0))?;
    control.set_gamma(file.as_fd());
    Ok(())
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
                client.add_output(registry, name, version, handle);
            }
            wl_registry::Event::GlobalRemove { name } => {
                if let Some(output) = client.outputs.remove(&name) {
                    if let Some(control) = output.control {
                        control.destroy();
                    }
                    if output.output.version() >= 3 {
                        output.output.release();
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Client {
    fn event(
        _: &mut Self,
        _: &wl_output::WlOutput,
        _: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrGammaControlManagerV1, ()> for Client {
    fn event(
        _: &mut Self,
        _: &ZwlrGammaControlManagerV1,
        _: <ZwlrGammaControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrGammaControlV1, u32> for Client {
    fn event(
        client: &mut Self,
        control: &ZwlrGammaControlV1,
        event: zwlr_gamma_control_v1::Event,
        name: &u32,
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let Some(output) = client.outputs.get_mut(name) else {
            return;
        };
        // An answer to a control since replaced.
        if output.control.as_ref() != Some(control) {
            return;
        }
        match event {
            zwlr_gamma_control_v1::Event::GammaSize { size } => {
                output.size = Some(size as usize);
                client.apply(*name, handle);
            }
            zwlr_gamma_control_v1::Event::Failed => {
                control.destroy();
                output.control = None;
                output.size = None;
                if !client.lost {
                    client.lost = true;
                    let _ = client.problems.send(
                        "the compositor refused an output's gamma: another night light, like wlsunset or hyprsunset, may hold it, or the output has none to set".to_owned(),
                    );
                }
            }
            _ => {}
        }
    }
}

//! The clipboard through `ext-data-control-v1`: a Wayland connection of the
//! module's own, which sees every new selection without focus, reads it,
//! and can take the selection over to serve an entry from the history.
//! Pasting types Ctrl+V through `zwp-virtual-keyboard-v1`.
//!
//! Like the compositor backend, it runs as a tokio task that waits for the
//! socket or for a request from the module.

use std::io::{ErrorKind, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::{self, ExtDataControlOfferV1},
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use zeroize::Zeroizing;

use crate::store::{Clip, Kind};

/// Offered with everything Mochi serves, so it knows its own selection.
pub const MARKER: &str = "application/x-mochi-clipboard";
/// What password managers offer with a secret, which isn't kept.
const SECRET_HINT: &str = "x-kde-passwordManagerHint";
/// Text formats, best first.
pub const TEXT: [&str; 5] = [
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
    "TEXT",
];
/// How long an app gets to hand over what it copied.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// The formats of the selection Mochi serves, by MIME type.
pub type Formats = Vec<(String, Arc<Zeroizing<Vec<u8>>>)>;

#[derive(Debug)]
pub enum Request {
    /// Take the selection over and serve these.
    Serve(Formats),
    /// Type Ctrl+V, with Shift too when `shift`.
    Paste { shift: bool },
}

/// The running connection. Dropping every copy stops the task.
#[derive(Debug, Clone)]
pub struct Watcher {
    requests: mpsc::UnboundedSender<Request>,
}

impl Watcher {
    pub fn send(&self, request: Request) -> Result<(), String> {
        self.requests
            .send(request)
            .map_err(|_| "lost the clipboard connection".to_owned())
    }
}

/// Connects and starts watching. New copies go to `copied`; nothing is read
/// while `listening` is false, or when a copy is larger than `limit` bytes.
pub fn start(
    copied: mpsc::UnboundedSender<Clip>,
    listening: Arc<AtomicBool>,
    limit: usize,
) -> Result<Watcher, String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("cannot connect to the Wayland display: {error}"))?;
    let (globals, queue) = registry_queue_init::<Client>(&connection)
        .map_err(|error| format!("cannot read the Wayland globals: {error}"))?;
    let handle = queue.handle();
    let manager: ExtDataControlManagerV1 = globals
        .bind(&handle, 1..=1, ())
        .map_err(|_| "the compositor doesn't support ext-data-control-v1".to_owned())?;
    let seat: wl_seat::WlSeat = globals
        .bind(&handle, 1..=1, ())
        .map_err(|_| "the compositor has no seat".to_owned())?;
    // Made now rather than at the first paste: a seat that had no keyboard
    // only gains one once the compositor has processed this, and keys sent
    // sooner reach nobody.
    let keyboard = virtual_keyboard(&globals, &seat, &handle);
    let device = manager.get_data_device(&seat, &handle, ());

    let (requests, receiver) = mpsc::unbounded_channel();
    let client = Client {
        manager,
        device,
        keyboard,
        started: Instant::now(),
        offer: None,
        source: None,
        copied,
        listening,
        limit,
        finished: false,
    };
    tokio::spawn(run(connection, queue, client, receiver));
    Ok(Watcher { requests })
}

async fn run(
    connection: Connection,
    mut queue: EventQueue<Client>,
    mut client: Client,
    mut requests: mpsc::UnboundedReceiver<Request>,
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
            tracing::error!(%error, "lost the clipboard's Wayland connection");
            return;
        }
        if client.finished {
            tracing::warn!("the compositor took the clipboard away");
            return;
        }
        if let Err(error) = queue.flush() {
            tracing::error!(%error, "lost the clipboard's Wayland connection");
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
                        tracing::error!(%error, "lost the clipboard's Wayland connection");
                        return;
                    }
                }
                ready.clear_ready();
            }
            request = requests.recv() => {
                drop(guard);
                match request {
                    Some(request) => client.perform(request, &handle),
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

struct Client {
    manager: ExtDataControlManagerV1,
    device: ExtDataControlDeviceV1,
    keyboard: Option<ZwpVirtualKeyboardV1>,
    /// Key events carry milliseconds from here.
    started: Instant,
    /// The current selection's offer.
    offer: Option<ExtDataControlOfferV1>,
    /// What Mochi serves, while it owns the selection.
    source: Option<ExtDataControlSourceV1>,
    copied: mpsc::UnboundedSender<Clip>,
    listening: Arc<AtomicBool>,
    limit: usize,
    finished: bool,
}

impl Client {
    fn perform(&mut self, request: Request, handle: &QueueHandle<Self>) {
        match request {
            Request::Serve(formats) => {
                let formats = Arc::new(formats);
                let source = self.manager.create_data_source(handle, formats.clone());
                for (mime, _) in formats.iter() {
                    source.offer(mime.clone());
                }
                source.offer(MARKER.into());
                self.device.set_selection(Some(&source));
                if let Some(old) = self.source.replace(source) {
                    old.destroy();
                }
            }
            Request::Paste { shift } => self.paste(shift),
        }
    }

    fn paste(&self, shift: bool) {
        let Some(keyboard) = &self.keyboard else {
            return;
        };
        let time = u32::try_from(self.started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let modifiers = CONTROL | if shift { SHIFT } else { 0 };
        keyboard.modifiers(modifiers, 0, 0, 0);
        keyboard.key(time, KEY_V, PRESSED);
        keyboard.key(time.saturating_add(1), KEY_V, RELEASED);
        keyboard.modifiers(0, 0, 0, 0);
    }

    /// A new selection: read it, unless it's Mochi's own, a secret, or
    /// nothing worth keeping.
    fn selection(&mut self, offer: Option<ExtDataControlOfferV1>) {
        if let Some(old) = std::mem::replace(&mut self.offer, offer.clone()) {
            old.destroy();
        }
        let Some(offer) = offer else { return };
        if !self.listening.load(Ordering::Relaxed) {
            return;
        }
        let mimes = offer
            .data::<Mutex<Vec<String>>>()
            .and_then(|mimes| mimes.lock().ok().map(|mimes| mimes.clone()))
            .unwrap_or_default();
        let Some((kind, wanted)) = wanted(&mimes) else {
            if mimes.iter().any(|mime| mime == SECRET_HINT) {
                tracing::debug!("skipped a copy marked secret");
            }
            return;
        };

        let mut pipes = Vec::with_capacity(wanted.len());
        for mime in wanted {
            match pipe() {
                Ok((read, write)) => {
                    // The request takes a copy of the write end, so ours
                    // closes here and the reader sees the app's end close.
                    offer.receive(mime.clone(), write.as_fd());
                    pipes.push((mime, read));
                }
                Err(error) => {
                    tracing::warn!(%error, "cannot read the clipboard");
                    return;
                }
            }
        }
        let copied = self.copied.clone();
        let limit = self.limit;
        tokio::spawn(async move {
            match read_all(pipes, limit).await {
                Ok(formats) => {
                    let main = &formats[0].1;
                    if kind == Kind::Text && main.iter().all(u8::is_ascii_whitespace) {
                        return;
                    }
                    let _ = copied.send(Clip { kind, formats });
                }
                Err(error) => tracing::info!(%error, "didn't keep a copy"),
            }
        });
    }
}

/// What to read from an offer with these MIME types, if anything: the best
/// text format and HTML alongside, or else an image, PNG first. Nothing for
/// Mochi's own selection or a secret.
fn wanted(mimes: &[String]) -> Option<(Kind, Vec<String>)> {
    let has = |wanted: &str| mimes.iter().any(|mime| mime == wanted);
    if has(MARKER) || has(SECRET_HINT) {
        return None;
    }
    if let Some(text) = TEXT.iter().find(|text| has(text)) {
        let mut wanted = vec![(*text).to_owned()];
        if has("text/html") {
            wanted.push("text/html".into());
        }
        return Some((Kind::Text, wanted));
    }
    let image = mimes
        .iter()
        .find(|mime| *mime == "image/png")
        .or_else(|| mimes.iter().find(|mime| mime.starts_with("image/")))?;
    Some((Kind::Image, vec![image.clone()]))
}

/// Reads every pipe to its end, the main format first. Text is stored as
/// UTF-8 whatever its name was.
async fn read_all(
    pipes: Vec<(String, OwnedFd)>,
    limit: usize,
) -> Result<Vec<(String, Zeroizing<Vec<u8>>)>, String> {
    let mut formats = Vec::with_capacity(pipes.len());
    let mut total = 0;
    for (mime, read) in pipes {
        let receiver = tokio::net::unix::pipe::Receiver::from_owned_fd(read)
            .map_err(|error| format!("cannot read the clipboard: {error}"))?;
        let mut data = Zeroizing::new(Vec::new());
        let budget = (limit - total + 1) as u64;
        let read =
            tokio::time::timeout(READ_TIMEOUT, receiver.take(budget).read_to_end(&mut data)).await;
        match read {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Err(format!("cannot read the clipboard: {error}")),
            Err(_) => return Err(format!("the app didn't hand over {mime} in time")),
        }
        total += data.len();
        if total > limit {
            return Err(format!("larger than {} MiB", limit >> 20));
        }
        let mime = if TEXT.contains(&mime.as_str()) {
            TEXT[0].to_owned()
        } else {
            mime
        };
        formats.push((mime, data));
    }
    Ok(formats)
}

/// A pipe whose read end doesn't block, for tokio; the write end goes to
/// the app as it is.
fn pipe() -> std::io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0; 2];
    // SAFETY: `fds` has room for the two descriptors pipe2 writes.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: pipe2 succeeded, so both are open descriptors nobody else owns.
    let (read, write) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    // SAFETY: fcntl on a descriptor we own, with flags it just returned.
    unsafe {
        let flags = libc::fcntl(read.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(read.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    Ok((read, write))
}

/// A virtual keyboard with Mochi's keymap, when the compositor has them.
fn virtual_keyboard(
    globals: &wayland_client::globals::GlobalList,
    seat: &wl_seat::WlSeat,
    handle: &QueueHandle<Client>,
) -> Option<ZwpVirtualKeyboardV1> {
    let Ok(keyboards) = globals.bind::<ZwpVirtualKeyboardManagerV1, _, _>(handle, 1..=1, ()) else {
        tracing::info!("no zwp-virtual-keyboard-v1: picking an entry only copies it");
        return None;
    };
    let keyboard = keyboards.create_virtual_keyboard(seat, handle, ());
    match keymap() {
        Ok((fd, size)) => {
            keyboard.keymap(1, fd.as_fd(), size);
            Some(keyboard)
        }
        Err(error) => {
            tracing::warn!(%error, "cannot make a keymap to paste with");
            keyboard.destroy();
            None
        }
    }
}

/// The xkb modifier masks, in the keymap's order.
const SHIFT: u32 = 1;
const CONTROL: u32 = 4;
/// The evdev code for V; xkb keycodes are 8 higher.
const KEY_V: u32 = 47;
const PRESSED: u32 = 1;
const RELEASED: u32 = 0;

/// Just the keys Mochi types, so it never depends on the user's layout.
const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "mochi" {
        minimum = 8;
        maximum = 255;
        <LFSH> = 50;
        <LCTL> = 37;
        <AB04> = 55;
    };
    xkb_types "mochi" { include "complete" };
    xkb_compatibility "mochi" { include "complete" };
    xkb_symbols "mochi" {
        key <LFSH> { [ Shift_L ] };
        key <LCTL> { [ Control_L ] };
        key <AB04> { [ v, V ] };
        modifier_map Shift { <LFSH> };
        modifier_map Control { <LCTL> };
    };
};
"#;

/// The keymap in a memory file, with its size counting the final NUL.
fn keymap() -> std::io::Result<(OwnedFd, u32)> {
    // SAFETY: a valid NUL-terminated name and flags.
    let fd = unsafe { libc::memfd_create(c"mochi-keymap".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: memfd_create returned a new descriptor nobody else owns.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.write_all(KEYMAP.as_bytes())?;
    file.write_all(&[0])?;
    Ok((file.into(), KEYMAP.len() as u32 + 1))
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Client {
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

impl Dispatch<ExtDataControlManagerV1, ()> for Client {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: <ExtDataControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for Client {
    fn event(
        client: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::Selection { id } => client.selection(id),
            ext_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } => {
                // Highlighted text isn't kept.
                offer.destroy();
            }
            ext_data_control_device_v1::Event::Finished => client.finished = true,
            _ => {}
        }
    }

    event_created_child!(Client, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, Mutex::new(Vec::new())),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, Mutex<Vec<String>>> for Client {
    fn event(
        _: &mut Self,
        _: &ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        mimes: &Mutex<Vec<String>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event
            && let Ok(mut mimes) = mimes.lock()
        {
            mimes.push(mime_type);
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, Arc<Formats>> for Client {
    fn event(
        client: &mut Self,
        source: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        formats: &Arc<Formats>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                let data = formats
                    .iter()
                    .find(|(mime, _)| *mime == mime_type)
                    .map(|(_, data)| data.clone());
                // Writing can wait on the app; off the event loop.
                tokio::task::spawn_blocking(move || {
                    let mut file = std::fs::File::from(fd);
                    if let Some(data) = data
                        && let Err(error) = file.write_all(&data)
                    {
                        tracing::debug!(%error, "an app stopped reading the clipboard");
                    }
                });
            }
            ext_data_control_source_v1::Event::Cancelled => {
                // Someone else copied.
                if client.source.as_ref() == Some(source) {
                    client.source = None;
                }
                source.destroy();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for Client {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardManagerV1,
        _: <ZwpVirtualKeyboardManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for Client {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardV1,
        _: <ZwpVirtualKeyboardV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// The text formats Mochi offers for a text entry, all with the same bytes.
pub fn text_formats(text: Arc<Zeroizing<Vec<u8>>>) -> Formats {
    TEXT.iter()
        .map(|mime| ((*mime).to_owned(), text.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mimes(list: &[&str]) -> Vec<String> {
        list.iter().map(|mime| (*mime).to_owned()).collect()
    }

    #[test]
    fn picks_text_with_html_then_images() {
        assert_eq!(
            wanted(&mimes(&["text/html", "TEXT", "text/plain;charset=utf-8"])),
            Some((
                Kind::Text,
                mimes(&["text/plain;charset=utf-8", "text/html"])
            ))
        );
        assert_eq!(
            wanted(&mimes(&["image/jpeg", "image/png", "text/html"])),
            Some((Kind::Image, mimes(&["image/png"])))
        );
        assert_eq!(
            wanted(&mimes(&["image/bmp"])),
            Some((Kind::Image, mimes(&["image/bmp"])))
        );
        assert_eq!(wanted(&mimes(&["application/x-whatever"])), None);
    }

    #[test]
    fn skips_secrets_and_its_own_selection() {
        assert_eq!(wanted(&mimes(&["text/plain", SECRET_HINT])), None);
        assert_eq!(wanted(&mimes(&["text/plain", MARKER])), None);
    }
}

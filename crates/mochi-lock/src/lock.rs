//! The session lock: `ext-session-lock-v1` on the compositor, a lock
//! surface on every monitor, the keyboard, and the password checks.
//!
//! The compositor shows nothing but these surfaces, and gives input to
//! nothing else, from the moment it says `locked` until [`Lock::unlock`],
//! which takes the [`Granted`] only a password PAM accepted makes. If this
//! process dies before that, the screen stays locked: the protocol says so,
//! and nothing here unlocks on the way out (`main` aborts on a panic, and a
//! dropped lock is destroyed, not unlocked).

use std::time::{Duration, Instant};

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop::channel::{self, Sender};
use smithay_client_toolkit::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopHandle};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::{
    wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface,
};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::keyboard::{
    KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::session_lock::{
    SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
    SessionLockSurfaceConfigure,
};
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_output, delegate_registry, delegate_seat,
    delegate_session_lock, delegate_shm, registry_handlers,
};

use crate::auth::{self, Granted, Refused};
use crate::config::Config;
use crate::face::{self, Clock, Face};
use crate::state::{Effect, Key, State};

/// How the process ends, for mochid.
pub mod exit {
    /// The password was right and the screen is unlocked.
    pub const UNLOCKED: i32 = 0;
    /// Something broke, like the compositor going away. Locked or not,
    /// mochid starts the lock again.
    pub const BROKEN: i32 = 1;
    /// The compositor refused the lock: it has no session lock protocol, or
    /// another locker holds it.
    pub const REFUSED: i32 = 2;
    /// No way to unlock, so no lock: the PAM service is missing, or the
    /// user is unknown.
    pub const UNSAFE: i32 = 3;
}

/// Tells mochid what happened, one JSON object per line on stdout.
#[allow(clippy::print_stdout)]
pub fn report(event: &str, reason: Option<&str>) {
    use std::io::Write;
    let line = serde_json::json!({ "event": event, "reason": reason });
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// Locks the session and waits for the right password. Returns the exit
/// code: [`exit::UNLOCKED`] only once it's unlocked.
pub fn run(config: Config, marker: Option<std::path::PathBuf>) -> i32 {
    let Some(user) = auth::user() else {
        report("refused", Some("can't tell who runs the lock"));
        return exit::UNSAFE;
    };
    if !auth::has_service(&config.pam_service) {
        let reason = format!(
            "there is no PAM service {:?} in {}, so the screen could never be unlocked",
            config.pam_service,
            auth::config_dir().display()
        );
        report("refused", Some(&reason));
        return exit::UNSAFE;
    }

    let Ok(connection) = Connection::connect_to_env() else {
        report("broken", Some("can't reach the compositor"));
        return exit::BROKEN;
    };
    let Ok((globals, queue)) = registry_queue_init::<Lock>(&connection) else {
        report("broken", Some("can't read the compositor's globals"));
        return exit::BROKEN;
    };
    let qh = queue.handle();
    let Ok(mut event_loop) = EventLoop::<Lock>::try_new() else {
        report("broken", Some("can't start the event loop"));
        return exit::BROKEN;
    };
    let (Ok(compositor), Ok(shm)) = (
        CompositorState::bind(&globals, &qh),
        Shm::bind(&globals, &qh),
    ) else {
        report(
            "broken",
            Some("the compositor lacks wl_compositor or wl_shm"),
        );
        return exit::BROKEN;
    };
    let Ok(pool) = SlotPool::new(1, &shm) else {
        report("broken", Some("can't share memory with the compositor"));
        return exit::BROKEN;
    };
    let (checks, results) = channel::channel::<Result<Granted, Refused>>();

    let session_state = SessionLockState::new(&globals, &qh);
    let mut lock = Lock {
        connection: connection.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        compositor,
        shm,
        pool,
        session: None,
        surfaces: Vec::new(),
        keyboard: None,
        loop_handle: event_loop.handle(),
        state: State::default(),
        face: Face::new(&config),
        clock: face::clock(&config.hours),
        config,
        user,
        checks,
        secured: false,
        marker,
        exit: None,
    };

    // Before anything draws.
    let Ok(session) = session_state.lock(&qh) else {
        report("refused", Some("the compositor has no ext-session-lock-v1"));
        return exit::REFUSED;
    };
    lock.session = Some(session);
    for output in lock.outputs.outputs() {
        lock.cover(&output, &qh);
    }

    let inserted = WaylandSource::new(connection, queue)
        .insert(event_loop.handle())
        .is_ok()
        && event_loop
            .handle()
            .insert_source(results, |event, (), lock| {
                if let channel::Event::Msg(result) = event {
                    lock.checked(result);
                }
            })
            .is_ok()
        && event_loop
            .handle()
            .insert_source(
                Timer::from_duration(Duration::from_secs(1)),
                |_, (), lock| {
                    lock.tick();
                    TimeoutAction::ToDuration(Duration::from_secs(1))
                },
            )
            .is_ok();
    if !inserted {
        report("broken", Some("can't watch the compositor"));
        return exit::BROKEN;
    }

    while lock.exit.is_none() {
        if event_loop.dispatch(None, &mut lock).is_err() {
            report("broken", Some("lost the compositor"));
            return exit::BROKEN;
        }
    }
    lock.exit.unwrap_or(exit::BROKEN)
}

struct Covered {
    surface: SessionLockSurface,
    output: wl_output::WlOutput,
    /// In logical pixels, once the compositor said.
    size: Option<(u32, u32)>,
}

struct Lock {
    connection: Connection,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    compositor: CompositorState,
    shm: Shm,
    pool: SlotPool,
    session: Option<SessionLock>,
    surfaces: Vec<Covered>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    loop_handle: LoopHandle<'static, Lock>,
    state: State,
    face: Face,
    clock: Clock,
    config: Config,
    user: String,
    checks: Sender<Result<Granted, Refused>>,
    /// The compositor said `locked`: only then do keys count.
    secured: bool,
    /// Where to say a lock is on, with this process's id, for mochid to find
    /// it again after a restart: written on `locked`, removed on unlocking.
    marker: Option<std::path::PathBuf>,
    exit: Option<i32>,
}

impl Lock {
    /// A lock surface on a monitor, for as long as it's there.
    fn cover(&mut self, output: &wl_output::WlOutput, qh: &QueueHandle<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        if self
            .surfaces
            .iter()
            .any(|covered| &covered.output == output)
        {
            return;
        }
        let surface = self.compositor.create_surface(qh);
        let surface = session.create_lock_surface(surface, output, qh);
        self.surfaces.push(Covered {
            surface,
            output: output.clone(),
            size: None,
        });
    }

    fn scale(&self, output: &wl_output::WlOutput) -> i32 {
        self.outputs
            .info(output)
            .map_or(1, |info| info.scale_factor.max(1))
    }

    fn draw_all(&mut self) {
        for index in 0..self.surfaces.len() {
            self.draw(index);
        }
    }

    fn draw(&mut self, index: usize) {
        let Some(covered) = self.surfaces.get(index) else {
            return;
        };
        let Some((width, height)) = covered.size else {
            return;
        };
        let scale = self.scale(&covered.output);
        let (pixel_width, pixel_height) = (width * scale as u32, height * scale as u32);
        let stride = pixel_width as i32 * 4;
        let Ok((buffer, canvas)) = self.pool.create_buffer(
            pixel_width as i32,
            pixel_height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) else {
            return;
        };
        let size = face::Size {
            width: pixel_width,
            height: pixel_height,
            scale: scale as f32,
        };
        self.face
            .draw(size, &self.state, &self.clock, Instant::now(), canvas);
        let surface = self.surfaces[index].surface.wl_surface();
        surface.set_buffer_scale(scale);
        surface.damage_buffer(0, 0, pixel_width as i32, pixel_height as i32);
        if buffer.attach_to(surface).is_ok() {
            surface.commit();
        }
    }

    fn key(&mut self, event: &KeyEvent, modifiers: Modifiers) {
        if !self.secured {
            return;
        }
        let key = match event.keysym {
            Keysym::Return | Keysym::KP_Enter => Key::Enter,
            Keysym::BackSpace if modifiers.ctrl => Key::Clear,
            Keysym::BackSpace => Key::Backspace,
            Keysym::Escape => Key::Clear,
            Keysym::u | Keysym::U if modifiers.ctrl => Key::Clear,
            // Shortcuts type nothing.
            _ if modifiers.ctrl || modifiers.alt || modifiers.logo => return,
            _ => match &event.utf8 {
                Some(text) if !text.is_empty() => Key::Text(text.clone()),
                _ => return,
            },
        };
        match self.state.key(key, Instant::now()) {
            Effect::Nothing => {}
            Effect::Redraw => self.draw_all(),
            Effect::Check(password) => {
                self.draw_all();
                let (service, user) = (self.config.pam_service.clone(), self.user.clone());
                let checks = self.checks.clone();
                let started = std::thread::Builder::new()
                    .name("pam".into())
                    .spawn(move || {
                        let _ = checks.send(auth::check(&service, &user, password));
                    });
                if started.is_err() {
                    self.state
                        .refused(vec!["Couldn't check the password".into()], Instant::now());
                    self.draw_all();
                }
            }
        }
    }

    fn checked(&mut self, result: Result<Granted, Refused>) {
        match result {
            Ok(granted) => self.unlock(granted),
            Err(refused) => {
                self.state.refused(refused.messages, Instant::now());
                self.draw_all();
            }
        }
    }

    /// The only way the screen unlocks: with what PAM granted.
    fn unlock(&mut self, _granted: Granted) {
        if let Some(session) = self.session.take() {
            session.unlock();
        }
        // The unlock reaches the compositor before this process goes.
        let _ = self.connection.roundtrip();
        if let Some(marker) = &self.marker {
            let _ = std::fs::remove_file(marker);
        }
        report("unlocked", None);
        self.exit = Some(exit::UNLOCKED);
    }

    fn tick(&mut self) {
        let clock = face::clock(&self.config.hours);
        let changed = clock.time != self.clock.time || clock.date != self.clock.date;
        self.clock = clock;
        let waited = self.state.tick(Instant::now());
        let waiting = matches!(self.state.phase, crate::state::Phase::Waiting(_));
        if changed || waited || waiting {
            self.draw_all();
        }
    }
}

impl SessionLockHandler for Lock {
    fn locked(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        self.secured = true;
        if let Some(marker) = &self.marker {
            let written = marker
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| {
                    let partial = marker.with_extension("partial");
                    std::fs::write(&partial, std::process::id().to_string())?;
                    std::fs::rename(partial, marker)
                });
            if let Err(error) = written {
                report(
                    "warning",
                    Some(&format!("can't write {}: {error}", marker.display())),
                );
            }
        }
        report("locked", None);
        self.draw_all();
    }

    fn finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        // Refused, most often because another locker holds the lock. Not
        // unlocked: the compositor decides that, and says so to whoever
        // holds it.
        self.session = None;
        report(
            "refused",
            Some(if self.secured {
                "the compositor ended the lock"
            } else {
                "the compositor refused the lock: another locker may hold it"
            }),
        );
        self.exit = Some(exit::REFUSED);
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _: u32,
    ) {
        let Some(index) = self
            .surfaces
            .iter()
            .position(|covered| covered.surface.wl_surface() == surface.wl_surface())
        else {
            return;
        };
        self.surfaces[index].size = Some(configure.new_size);
        self.draw(index);
    }
}

impl OutputHandler for Lock {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    // A monitor plugged in while locked is covered too.
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.cover(&output, qh);
    }

    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(index) = self
            .surfaces
            .iter()
            .position(|covered| covered.output == output)
        {
            self.draw(index);
        }
    }

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.surfaces.retain(|covered| covered.output != output);
    }
}

impl CompositorHandler for Lock {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _: i32,
    ) {
        if let Some(index) = self
            .surfaces
            .iter()
            .position(|covered| covered.surface.wl_surface() == surface)
        {
            self.draw(index);
        }
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl SeatHandler for Lock {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability != Capability::Keyboard || self.keyboard.is_some() {
            return;
        }
        // Held keys repeat, like Backspace.
        let keyboard = self.seats.get_keyboard_with_repeat(
            qh,
            &seat,
            None,
            self.loop_handle.clone(),
            Box::new(|lock: &mut Lock, _, event| {
                let modifiers = lock.modifiers();
                lock.key(&event, modifiers);
            }),
        );
        self.keyboard = keyboard.ok();
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard
            && let Some(keyboard) = self.keyboard.take()
        {
            keyboard.release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl Lock {
    /// The modifiers held, as the face last heard.
    fn modifiers(&self) -> Modifiers {
        MODIFIERS.with(std::cell::Cell::get)
    }
}

thread_local! {
    // The keyboard's modifiers, kept apart from `Lock` so the repeat
    // callback can read them.
    static MODIFIERS: std::cell::Cell<Modifiers> = std::cell::Cell::new(Modifiers::default());
}

impl KeyboardHandler for Lock {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        let modifiers = self.modifiers();
        self.key(&event, modifiers);
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        let modifiers = self.modifiers();
        self.key(&event, modifiers);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        MODIFIERS.with(|held| held.set(modifiers));
        if self.state.caps_lock != modifiers.caps_lock {
            self.state.caps_lock = modifiers.caps_lock;
            self.draw_all();
        }
    }
}

impl ShmHandler for Lock {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Lock {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(Lock);
delegate_output!(Lock);
delegate_seat!(Lock);
delegate_keyboard!(Lock);
delegate_shm!(Lock);
delegate_session_lock!(Lock);
delegate_registry!(Lock);

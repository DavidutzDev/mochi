//! The audio server over the PulseAudio protocol (served by pipewire-pulse on
//! PipeWire systems): every output, input and app playing sound, and the
//! commands that change them.
//!
//! libpulse is callback-based and single-threaded, so it runs its own
//! mainloop on a dedicated thread. It sends a [`Snapshot`] of everything
//! after each change, and takes [`Command`]s through a channel; a byte on a
//! socket wakes the mainloop to read them. When the server goes away, the
//! thread reports [`Report::Lost`] and reconnects with backoff.
//!
//! While a view of the mixer is open, it also runs the peak meters of
//! [`crate::meter`], which leave what they measure in a shared map.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::introspect::{SinkInfo, SinkInputInfo, SourceInfo};
use libpulse_binding::context::subscribe::{Facility, InterestMaskSet};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::def::{SinkState, SourceState};
use libpulse_binding::mainloop::api::Mainloop as _;
use libpulse_binding::mainloop::events::io::FlagSet as IoFlags;
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use libpulse_binding::proplist::{Proplist, properties};
use libpulse_binding::volume::{ChannelVolumes, Volume};
use tokio::sync::mpsc::UnboundedSender;

use crate::meter::{Meters, Peaks};

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(30);
/// A connection that lasted this long resets the backoff.
const STABLE: Duration = Duration::from_secs(10);

/// Everything the mixer shows, as the server last reported it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub default_sink: Option<String>,
    pub default_source: Option<String>,
    pub sinks: Vec<Device>,
    /// Microphones and other inputs; the monitors of outputs are left out.
    pub sources: Vec<Device>,
    /// Apps playing sound, one entry per stream.
    pub streams: Vec<Stream>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub index: u32,
    pub name: String,
    pub description: String,
    /// Percent, 100 for the device's normal level.
    pub volume: u32,
    pub muted: bool,
    /// What it plugs into, for the icon: `headset`, `display` or `speakers`.
    pub kind: &'static str,
    /// The source that hears what an output plays; `None` for inputs.
    pub monitor: Option<String>,
    /// Playing or recording for some app now, rather than idle.
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    pub index: u32,
    /// The index of the output it plays through.
    pub sink: u32,
    /// The app's name, like "Spotify" or "Firefox".
    pub app: String,
    /// What it plays, like a tab's title.
    pub title: String,
    /// An icon theme name, from the app or its binary.
    pub icon: Option<String>,
    pub volume: u32,
    pub muted: bool,
    /// Paused streams stay connected but send nothing.
    pub corked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    Snapshot(Snapshot),
    /// The connection was lost; nothing is known until the next snapshot.
    Lost,
}

/// What a command changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Sink(String),
    Source(String),
    Stream(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Sets the loudest channel to this percent, keeping the balance.
    Volume(Target, u32),
    Mute(Target, bool),
    DefaultSink(String),
    DefaultSource(String),
    /// Moves an app's stream to the output with this name.
    Move(u32, String),
    /// Runs the peak meters, or stops them.
    Meters(bool),
}

/// Sends commands to the audio thread.
#[derive(Debug)]
pub struct Handle {
    commands: mpsc::Sender<Command>,
    wake: UnixStream,
}

impl Handle {
    pub fn send(&self, command: Command) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "the audio thread stopped".to_owned())?;
        // A full socket already has a wakeup waiting.
        let _ = (&self.wake).write(&[1]);
        Ok(())
    }
}

pub fn spawn(reports: UnboundedSender<Report>, peaks: Peaks) -> std::io::Result<Handle> {
    let (wake, woken) = UnixStream::pair()?;
    wake.set_nonblocking(true)?;
    woken.set_nonblocking(true)?;
    let (commands, received) = mpsc::channel();
    thread::Builder::new().name("audio".into()).spawn(move || {
        let inbox = Rc::new(Inbox {
            received,
            woken,
            metering: Cell::new(false),
        });
        run(&reports, &inbox, &peaks);
    })?;
    Ok(Handle { commands, wake })
}

/// The thread's end of [`Handle`].
struct Inbox {
    received: mpsc::Receiver<Command>,
    woken: UnixStream,
    /// Whether the meters should run, kept across connections.
    metering: Cell<bool>,
}

impl Inbox {
    fn drain(&self) -> Vec<Command> {
        let mut bytes = [0; 64];
        while matches!((&self.woken).read(&mut bytes), Ok(read) if read > 0) {}
        self.received.try_iter().collect()
    }
}

fn run(reports: &UnboundedSender<Report>, inbox: &Rc<Inbox>, peaks: &Peaks) {
    let mut retry = FIRST_RETRY;
    loop {
        let started = Instant::now();
        match session(reports, inbox, peaks) {
            Ok(()) => return,
            Err(error) => {
                if started.elapsed() > STABLE {
                    retry = FIRST_RETRY;
                }
                tracing::warn!(%error, retry = ?retry, "audio server connection lost");
                if reports.send(Report::Lost).is_err() {
                    return;
                }
                thread::sleep(retry);
                retry = (retry * 2).min(MAX_RETRY);
                // Commands for the old connection are stale, but the next
                // one should still meter or not.
                for command in inbox.drain() {
                    if let Command::Meters(on) = command {
                        inbox.metering.set(on);
                    }
                }
            }
        }
    }
}

/// What the callbacks share: the context, and the snapshot being built.
struct Link {
    context: Weak<RefCell<Context>>,
    reports: UnboundedSender<Report>,
    building: RefCell<Building>,
    /// The channel volumes of the last snapshot, to keep the balance when
    /// setting a level.
    volumes: RefCell<HashMap<Target, ChannelVolumes>>,
    /// The last snapshot, for the meters to follow.
    last: RefCell<Snapshot>,
    meters: RefCell<Meters>,
    inbox: Rc<Inbox>,
}

#[derive(Default)]
struct Building {
    /// Answers still to come; 0 when no refresh runs.
    remaining: u8,
    /// Something changed while building: refresh again after.
    again: bool,
    snapshot: Snapshot,
    volumes: HashMap<Target, ChannelVolumes>,
}

/// One connection. Returns `Ok` when the module stopped listening, and `Err`
/// when the connection failed or dropped.
fn session(
    reports: &UnboundedSender<Report>,
    inbox: &Rc<Inbox>,
    peaks: &Peaks,
) -> Result<(), String> {
    let mut mainloop = Mainloop::new().ok_or("cannot create a libpulse mainloop")?;
    let mut proplist = Proplist::new().ok_or("cannot create a proplist")?;
    let _ = proplist.set_str(properties::APPLICATION_NAME, "Mochi");
    let context = Context::new_with_proplist(&mainloop, "Mochi", &proplist)
        .ok_or("cannot create a libpulse context")?;
    let context = Rc::new(RefCell::new(context));

    context
        .borrow_mut()
        .connect(None, FlagSet::NOFLAGS, None)
        .map_err(|error| format!("cannot connect: {error}"))?;

    loop {
        iterate(&mut mainloop)?;
        match context.borrow().get_state() {
            State::Ready => break,
            State::Failed | State::Terminated => {
                return Err("the server refused the connection".into());
            }
            _ => {}
        }
    }
    tracing::info!("connected to the audio server");

    // Callbacks hold the link, which holds the context weakly: a strong
    // reference would keep the context alive through its own callbacks.
    let link = Rc::new(Link {
        context: Rc::downgrade(&context),
        reports: reports.clone(),
        building: RefCell::default(),
        volumes: RefCell::default(),
        last: RefCell::default(),
        meters: RefCell::new(Meters::new(Arc::clone(peaks))),
        inbox: Rc::clone(inbox),
    });

    let subscribed = Rc::clone(&link);
    context
        .borrow_mut()
        .set_subscribe_callback(Some(Box::new(move |facility, _, _| {
            if matches!(
                facility,
                Some(Facility::Sink | Facility::Source | Facility::SinkInput | Facility::Server)
            ) {
                refresh(&subscribed);
            }
        })));
    context.borrow_mut().subscribe(
        InterestMaskSet::SINK
            | InterestMaskSet::SOURCE
            | InterestMaskSet::SINK_INPUT
            | InterestMaskSet::SERVER,
        |_| {},
    );

    let woken = Rc::clone(&link);
    let commands = Rc::clone(inbox);
    let _wakeup = mainloop
        .new_io_event(
            inbox.woken.as_raw_fd(),
            IoFlags::INPUT,
            Box::new(move |_, _, _| {
                for command in commands.drain() {
                    apply(&woken, command);
                }
            }),
        )
        .ok_or("cannot watch for commands")?;

    refresh(&link);

    loop {
        iterate(&mut mainloop)?;
        if reports.is_closed() {
            context.borrow_mut().set_subscribe_callback(None);
            return Ok(());
        }
        if matches!(
            context.borrow().get_state(),
            State::Failed | State::Terminated
        ) {
            return Err("the server closed the connection".into());
        }
    }
}

fn iterate(mainloop: &mut Mainloop) -> Result<(), String> {
    match mainloop.iterate(true) {
        IterateResult::Success(_) => Ok(()),
        IterateResult::Quit(_) => Err("the mainloop quit".into()),
        IterateResult::Err(error) => Err(format!("mainloop error: {error}")),
    }
}

/// Asks for everything again, and sends a snapshot once all of it came.
/// Changes during a refresh start another one after it.
fn refresh(link: &Rc<Link>) {
    {
        let mut building = link.building.borrow_mut();
        if building.remaining > 0 {
            building.again = true;
            return;
        }
        *building = Building {
            remaining: 4,
            ..Building::default()
        };
    }
    let Some(context) = link.context.upgrade() else {
        return;
    };
    let introspect = context.borrow().introspect();

    let server = Rc::clone(link);
    introspect.get_server_info(move |info| {
        {
            let mut building = server.building.borrow_mut();
            building.snapshot.default_sink = info.default_sink_name.as_deref().map(str::to_owned);
            building.snapshot.default_source =
                info.default_source_name.as_deref().map(str::to_owned);
        }
        answered(&server);
    });

    let sinks = Rc::clone(link);
    introspect.get_sink_info_list(move |result| match result {
        ListResult::Item(sink) => {
            let device = sink_device(sink);
            let mut building = sinks.building.borrow_mut();
            building
                .volumes
                .insert(Target::Sink(device.name.clone()), sink.volume);
            building.snapshot.sinks.push(device);
        }
        ListResult::End | ListResult::Error => answered(&sinks),
    });

    let sources = Rc::clone(link);
    introspect.get_source_info_list(move |result| match result {
        ListResult::Item(source) => {
            let Some(device) = source_device(source) else {
                return;
            };
            let mut building = sources.building.borrow_mut();
            building
                .volumes
                .insert(Target::Source(device.name.clone()), source.volume);
            building.snapshot.sources.push(device);
        }
        ListResult::End | ListResult::Error => answered(&sources),
    });

    let streams = Rc::clone(link);
    introspect.get_sink_input_info_list(move |result| match result {
        ListResult::Item(input) => {
            let Some(stream) = stream(input) else {
                return;
            };
            let mut building = streams.building.borrow_mut();
            building
                .volumes
                .insert(Target::Stream(stream.index), input.volume);
            building.snapshot.streams.push(stream);
        }
        ListResult::End | ListResult::Error => answered(&streams),
    });
}

/// One of the four answers came.
fn answered(link: &Rc<Link>) {
    let (snapshot, volumes, again) = {
        let mut building = link.building.borrow_mut();
        building.remaining = building.remaining.saturating_sub(1);
        if building.remaining > 0 {
            return;
        }
        let building = std::mem::take(&mut *building);
        (building.snapshot, building.volumes, building.again)
    };
    *link.volumes.borrow_mut() = volumes;
    *link.last.borrow_mut() = snapshot.clone();
    meter(link);
    let _ = link.reports.send(Report::Snapshot(snapshot));
    if again {
        refresh(link);
    }
}

/// Opens or closes meters to match the last snapshot, or stops them all.
fn meter(link: &Link) {
    let Some(context) = link.context.upgrade() else {
        return;
    };
    let last = link.last.borrow();
    let snapshot = link.inbox.metering.get().then_some(&*last);
    link.meters
        .borrow_mut()
        .follow(&mut context.borrow_mut(), snapshot);
}

fn apply(link: &Link, command: Command) {
    let Some(context) = link.context.upgrade() else {
        return;
    };
    let mut introspect = context.borrow().introspect();
    match command {
        Command::Volume(target, percent) => {
            let Some(mut volumes) = link.volumes.borrow().get(&target).copied() else {
                tracing::debug!(?target, "no such audio device or stream");
                return;
            };
            volumes.scale(level(percent));
            match target {
                Target::Sink(name) => {
                    introspect.set_sink_volume_by_name(&name, &volumes, None);
                }
                Target::Source(name) => {
                    introspect.set_source_volume_by_name(&name, &volumes, None);
                }
                Target::Stream(index) => {
                    introspect.set_sink_input_volume(index, &volumes, None);
                }
            }
        }
        Command::Mute(target, muted) => match target {
            Target::Sink(name) => {
                introspect.set_sink_mute_by_name(&name, muted, None);
            }
            Target::Source(name) => {
                introspect.set_source_mute_by_name(&name, muted, None);
            }
            Target::Stream(index) => {
                introspect.set_sink_input_mute(index, muted, None);
            }
        },
        Command::DefaultSink(name) => {
            context.borrow_mut().set_default_sink(&name, |_| {});
        }
        Command::DefaultSource(name) => {
            context.borrow_mut().set_default_source(&name, |_| {});
        }
        Command::Move(index, sink) => {
            introspect.move_sink_input_by_name(index, &sink, None);
        }
        Command::Meters(on) => {
            link.inbox.metering.set(on);
            meter(link);
        }
    }
}

fn sink_device(sink: &SinkInfo) -> Device {
    let name = sink.name.as_deref().unwrap_or_default().to_owned();
    let port = sink
        .active_port
        .as_ref()
        .and_then(|port| port.name.as_deref())
        .unwrap_or_default();
    let form_factor = sink.proplist.get_str("device.form_factor");
    Device {
        index: sink.index,
        monitor: sink.monitor_source_name.as_deref().map(str::to_owned),
        running: sink.state == SinkState::Running,
        kind: kind(form_factor.as_deref(), port, &name),
        description: sink.description.as_deref().unwrap_or(&name).to_owned(),
        volume: percent(sink.volume.max()),
        muted: sink.mute,
        name,
    }
}

/// A monitor of an output is not a microphone, so it isn't listed.
fn source_device(source: &SourceInfo) -> Option<Device> {
    if source.monitor_of_sink.is_some() {
        return None;
    }
    let name = source.name.as_deref().unwrap_or_default().to_owned();
    let form_factor = source.proplist.get_str("device.form_factor");
    let kind = match form_factor.as_deref() {
        Some("headset" | "headphone" | "hands-free") => "headset",
        _ => "mic",
    };
    Some(Device {
        index: source.index,
        monitor: None,
        running: source.state == SourceState::Running,
        kind,
        description: source.description.as_deref().unwrap_or(&name).to_owned(),
        volume: percent(source.volume.max()),
        muted: source.mute,
        name,
    })
}

/// Streams without a volume of their own, like some system sounds, can't
/// be mixed, so they aren't listed.
fn stream(input: &SinkInputInfo) -> Option<Stream> {
    if !input.has_volume || !input.volume_writable {
        return None;
    }
    let property = |key: &str| {
        input
            .proplist
            .get_str(key)
            .filter(|value| !value.trim().is_empty())
    };
    let binary = property(properties::APPLICATION_PROCESS_BINARY);
    let app = property(properties::APPLICATION_NAME)
        .or_else(|| binary.clone())
        .or_else(|| input.name.as_deref().map(str::to_owned))
        .unwrap_or_else(|| "Unknown app".to_owned());
    let title = property(properties::MEDIA_NAME)
        .or_else(|| input.name.as_deref().map(str::to_owned))
        .unwrap_or_default();
    Some(Stream {
        index: input.index,
        sink: input.sink,
        icon: property(properties::APPLICATION_ICON_NAME)
            .or_else(|| binary.map(|binary| binary.to_lowercase())),
        // Apps often name the stream after themselves; that says nothing.
        title: if title == app { String::new() } else { title },
        app,
        volume: percent(input.volume.max()),
        muted: input.mute,
        corked: input.corked,
    })
}

/// Guesses what an output plugs into from what the server reports: the form
/// factor property, then the active port and device names.
fn kind(form_factor: Option<&str>, port: &str, name: &str) -> &'static str {
    match form_factor {
        Some("headset" | "headphone" | "hands-free") => return "headset",
        Some("speaker" | "internal" | "computer") => return "speakers",
        _ => {}
    }
    let words = format!("{port} {name}").to_ascii_lowercase();
    if ["hdmi", "displayport"]
        .iter()
        .any(|word| words.contains(word))
    {
        "display"
    } else if ["headphone", "headset", "bluez"]
        .iter()
        .any(|word| words.contains(word))
    {
        "headset"
    } else {
        "speakers"
    }
}

fn percent(volume: Volume) -> u32 {
    let ratio = f64::from(volume.0) / f64::from(Volume::NORMAL.0);
    (ratio * 100.0).round() as u32
}

fn level(percent: u32) -> Volume {
    let raw = u64::from(Volume::NORMAL.0) * u64::from(percent) / 100;
    Volume(
        u32::try_from(raw)
            .unwrap_or(Volume::MAX.0)
            .min(Volume::MAX.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_and_level_round_trip() {
        for value in [0, 1, 37, 50, 100, 150, 200, 300] {
            assert_eq!(percent(level(value)), value);
        }
        assert_eq!(level(100), Volume::NORMAL);
    }

    #[test]
    fn guesses_what_an_output_plugs_into() {
        assert_eq!(kind(Some("headset"), "", ""), "headset");
        assert_eq!(kind(None, "hdmi-output-0", "alsa_output.pci"), "display");
        assert_eq!(kind(None, "", "bluez_output.AA_BB.1"), "headset");
        assert_eq!(
            kind(None, "analog-output-lineout", "alsa_output.pci"),
            "speakers"
        );
    }
}

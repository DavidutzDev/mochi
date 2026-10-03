//! The default output and input, from the audio server over the PulseAudio
//! protocol (served by pipewire-pulse on PipeWire systems).
//!
//! libpulse is callback-based and single-threaded, so it runs its own
//! mainloop on a dedicated thread and sends state reports to the module over
//! a channel. When the server goes away, the thread reports `AudioReset` and
//! reconnects with backoff.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::thread;
use std::time::{Duration, Instant};

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::introspect::{SinkInfo, SourceInfo};
use libpulse_binding::context::subscribe::{Facility, InterestMaskSet};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use libpulse_binding::proplist::{Proplist, properties};
use libpulse_binding::volume::Volume;
use tokio::sync::mpsc::UnboundedSender;

use crate::notice::{Change, DeviceKind, Input, Output};

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(30);
/// A connection that lasted this long resets the backoff.
const STABLE: Duration = Duration::from_secs(10);

pub fn spawn(changes: UnboundedSender<Change>) -> std::io::Result<()> {
    thread::Builder::new()
        .name("osd-audio".into())
        .spawn(move || run(&changes))?;
    Ok(())
}

fn run(changes: &UnboundedSender<Change>) {
    let mut retry = FIRST_RETRY;
    loop {
        let started = Instant::now();
        match session(changes) {
            Ok(()) => return,
            Err(error) => {
                if started.elapsed() > STABLE {
                    retry = FIRST_RETRY;
                }
                tracing::warn!(%error, retry = ?retry, "audio server connection lost");
                if changes.send(Change::AudioReset).is_err() {
                    return;
                }
                thread::sleep(retry);
                retry = (retry * 2).min(MAX_RETRY);
            }
        }
    }
}

/// One connection. Returns `Ok` when the module stopped listening, and `Err`
/// when the connection failed or dropped.
fn session(changes: &UnboundedSender<Change>) -> Result<(), String> {
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

    // The callbacks hold a weak reference: a strong one would keep the
    // context alive through its own callback.
    let weak = Rc::downgrade(&context);
    let sender = changes.clone();
    context
        .borrow_mut()
        .set_subscribe_callback(Some(Box::new(move |facility, _, _| {
            if matches!(
                facility,
                Some(Facility::Sink | Facility::Source | Facility::Server)
            ) {
                query(&weak, &sender);
            }
        })));
    context.borrow_mut().subscribe(
        InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER,
        |_| {},
    );

    // The first answer becomes the module's baseline.
    query(&Rc::downgrade(&context), changes);

    loop {
        iterate(&mut mainloop)?;
        if changes.is_closed() {
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

/// Asks for the default devices, then reports each one's state.
fn query(context: &Weak<RefCell<Context>>, changes: &UnboundedSender<Change>) {
    let Some(strong) = context.upgrade() else {
        return;
    };
    let context = context.clone();
    let changes = changes.clone();
    strong.borrow().introspect().get_server_info(move |server| {
        let Some(context) = context.upgrade() else {
            return;
        };
        let introspector = context.borrow().introspect();

        match server.default_sink_name.as_deref() {
            Some(name) => {
                let changes = changes.clone();
                introspector.get_sink_info_by_name(name, move |result| {
                    if let ListResult::Item(sink) = result {
                        let _ = changes.send(Change::Output(Some(output(sink))));
                    }
                });
            }
            None => {
                let _ = changes.send(Change::Output(None));
            }
        }

        match server.default_source_name.as_deref() {
            Some(name) => {
                let changes = changes.clone();
                introspector.get_source_info_by_name(name, move |result| {
                    if let ListResult::Item(source) = result {
                        let _ = changes.send(Change::Input(input(source)));
                    }
                });
            }
            None => {
                let _ = changes.send(Change::Input(None));
            }
        }
    });
}

fn output(sink: &SinkInfo) -> Output {
    let name = sink.name.as_deref().unwrap_or_default().to_owned();
    let port = sink
        .active_port
        .as_ref()
        .and_then(|port| port.name.as_deref())
        .unwrap_or_default();
    let form_factor = sink.proplist.get_str("device.form_factor");
    Output {
        kind: DeviceKind::detect(form_factor.as_deref(), port, &name),
        description: sink.description.as_deref().unwrap_or(&name).to_owned(),
        volume: percent(sink.volume.avg()),
        muted: sink.mute,
        name,
    }
}

/// A monitor of an output is not a microphone, so it doesn't count.
fn input(source: &SourceInfo) -> Option<Input> {
    if source.monitor_of_sink.is_some() {
        return None;
    }
    Some(Input {
        name: source.name.as_deref().unwrap_or_default().to_owned(),
        muted: source.mute,
    })
}

fn percent(volume: Volume) -> u32 {
    let ratio = f64::from(volume.0) / f64::from(Volume::NORMAL.0);
    (ratio * 100.0).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_rounds_to_the_nearest() {
        assert_eq!(percent(Volume::NORMAL), 100);
        assert_eq!(percent(Volume(0)), 0);
        assert_eq!(percent(Volume(Volume::NORMAL.0 / 2)), 50);
        assert_eq!(percent(Volume(Volume::NORMAL.0 * 57 / 100 + 1)), 57);
        assert_eq!(percent(Volume(Volume::NORMAL.0 * 3 / 2)), 150);
    }
}

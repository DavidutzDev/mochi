//! Peak meters: a record stream with peak detection on whatever the mixer
//! shows a slider for. The server then sends one peak per 40 ms instead of
//! the audio itself. They only run while a view of the mixer is open.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use libpulse_binding::context::Context;
use libpulse_binding::def::BufferAttr;
use libpulse_binding::proplist::{Proplist, properties};
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::{FlagSet, PeekResult, State, Stream};

use crate::pulse::Snapshot;

/// The loudest peak per meter since the module last took them, from 0 to 1,
/// keyed as [`wanted`] names them.
pub type Peaks = Arc<Mutex<HashMap<String, f32>>>;

/// Takes the peaks gathered so far, leaving none.
pub fn take(peaks: &Peaks) -> HashMap<String, f32> {
    std::mem::take(&mut *peaks.lock().unwrap_or_else(PoisonError::into_inner))
}

/// What one meter listens to: a source, and for an app's stream, which
/// stream of that output's monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tap {
    pub source: String,
    pub stream: Option<u32>,
}

/// The meters the mixer shows: `output` and `input` for the devices in use,
/// and each app stream by its id. Only for devices that run and streams that
/// play: there is nothing to measure otherwise, and PipeWire doesn't finish
/// making a record stream on a device that sleeps, which then can't close.
/// So the input's meter moves only while an app records from it.
pub fn wanted(snapshot: &Snapshot) -> HashMap<String, Tap> {
    let mut taps = HashMap::new();
    let monitor =
        |sink: &crate::pulse::Device| sink.running.then(|| sink.monitor.clone()).flatten();
    if let Some(source) = snapshot
        .sinks
        .iter()
        .find(|sink| snapshot.default_sink.as_ref() == Some(&sink.name))
        .and_then(monitor)
    {
        taps.insert(
            "output".to_owned(),
            Tap {
                source,
                stream: None,
            },
        );
    }
    if let Some(source) = snapshot
        .sources
        .iter()
        .find(|source| snapshot.default_source.as_ref() == Some(&source.name))
        .filter(|source| source.running)
    {
        taps.insert(
            "input".to_owned(),
            Tap {
                source: source.name.clone(),
                stream: None,
            },
        );
    }
    for stream in snapshot.streams.iter().filter(|stream| !stream.corked) {
        if let Some(source) = snapshot
            .sinks
            .iter()
            .find(|sink| sink.index == stream.sink)
            .and_then(monitor)
        {
            taps.insert(
                stream.index.to_string(),
                Tap {
                    source,
                    stream: Some(stream.index),
                },
            );
        }
    }
    taps
}

/// The open meters, and those closing.
pub struct Meters {
    peaks: Peaks,
    open: HashMap<String, (Tap, Rc<RefCell<Stream>>)>,
    /// Streams the server hadn't made yet when they closed: libpulse can
    /// only disconnect them once it has. Kept until they're gone.
    closing: Vec<Rc<RefCell<Stream>>>,
}

impl Meters {
    pub fn new(peaks: Peaks) -> Self {
        Self {
            peaks,
            open: HashMap::new(),
            closing: Vec::new(),
        }
    }

    /// Opens the meters `snapshot` asks for and closes the rest, keeping
    /// those that listen to the same thing. Without a snapshot, closes them
    /// all.
    pub fn follow(&mut self, context: &mut Context, snapshot: Option<&Snapshot>) {
        let gone = |stream: &Rc<RefCell<Stream>>| {
            matches!(
                stream.borrow().get_state(),
                State::Terminated | State::Failed
            )
        };
        self.closing.retain(|stream| !gone(stream));
        let wanted = snapshot.map(wanted).unwrap_or_default();
        let mut closed = Vec::new();
        self.open.retain(|key, (tap, stream)| {
            // A meter whose stream the server ended opens again.
            let keep = wanted.get(key) == Some(tap) && !gone(stream);
            if !keep {
                closed.push(Rc::clone(stream));
            }
            keep
        });
        for stream in closed {
            self.close(stream);
        }
        if self.open.is_empty() {
            self.peaks
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clear();
        }
        for (key, tap) in wanted {
            if self.open.contains_key(&key) {
                continue;
            }
            match open(context, &self.peaks, &key, &tap) {
                Some(stream) => {
                    self.open.insert(key, (tap, stream));
                }
                None => tracing::debug!(key, source = tap.source, "cannot open a peak meter"),
            }
        }
    }

    fn close(&mut self, stream: Rc<RefCell<Stream>>) {
        let creating = {
            let mut meter = stream.borrow_mut();
            meter.set_read_callback(None);
            if meter.get_state() == State::Creating {
                let weak = Rc::downgrade(&stream);
                meter.set_state_callback(Some(Box::new(move || {
                    if let Some(stream) = weak.upgrade()
                        && let Ok(mut meter) = stream.try_borrow_mut()
                        && meter.get_state() == State::Ready
                    {
                        let _ = meter.disconnect();
                    }
                })));
                true
            } else {
                let _ = meter.disconnect();
                false
            }
        };
        if creating {
            self.closing.push(stream);
        }
    }
}

fn open(context: &mut Context, peaks: &Peaks, key: &str, tap: &Tap) -> Option<Rc<RefCell<Stream>>> {
    // One float per peak, 25 a second.
    let spec = Spec {
        format: Format::F32le,
        rate: 25,
        channels: 1,
    };
    let mut proplist = Proplist::new()?;
    let _ = proplist.set_str(properties::APPLICATION_ID, "mochi.meter");
    let mut stream = Stream::new_with_proplist(context, "Peak meter", &spec, None, &mut proplist)?;
    if let Some(index) = tap.stream {
        stream.set_monitor_stream(index).ok()?;
    }
    let stream = Rc::new(RefCell::new(stream));

    let weak = Rc::downgrade(&stream);
    let peaks = Arc::clone(peaks);
    let key = key.to_owned();
    stream
        .borrow_mut()
        .set_read_callback(Some(Box::new(move |_| {
            let Some(stream) = weak.upgrade() else {
                return;
            };
            let mut stream = stream.borrow_mut();
            let peak = match stream.peek() {
                Ok(PeekResult::Data(bytes)) => loudest(bytes),
                Ok(PeekResult::Hole(_)) => 0.0,
                Ok(PeekResult::Empty) | Err(_) => return,
            };
            let _ = stream.discard();
            let mut peaks = peaks.lock().unwrap_or_else(PoisonError::into_inner);
            match peaks.get_mut(&key) {
                Some(level) => *level = level.max(peak),
                None => {
                    peaks.insert(key.clone(), peak);
                }
            }
        })));

    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: 4,
    };
    let mut flags =
        FlagSet::PEAK_DETECT | FlagSet::ADJUST_LATENCY | FlagSet::DONT_INHIBIT_AUTO_SUSPEND;
    if tap.stream.is_some() {
        // When the app moves to another output, the next snapshot opens a
        // meter there.
        flags |= FlagSet::DONT_MOVE;
    }
    let connected = stream
        .borrow_mut()
        .connect_record(Some(&tap.source), Some(&attr), flags);
    if connected.is_err() {
        stream.borrow_mut().set_read_callback(None);
        return None;
    }
    Some(stream)
}

/// The loudest of some float samples.
fn loudest(bytes: &[u8]) -> f32 {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|sample| f32::from_le_bytes(*sample).abs())
        .fold(0.0, f32::max)
        .min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse::{Device, Stream as AppStream};

    fn sink(index: u32, name: &str) -> Device {
        Device {
            index,
            name: name.into(),
            description: name.into(),
            volume: 100,
            muted: false,
            kind: "speakers",
            monitor: Some(format!("{name}.monitor")),
            running: true,
        }
    }

    #[test]
    fn listens_to_the_devices_in_use_and_each_app() {
        let mut snapshot = Snapshot {
            default_sink: Some("speakers".into()),
            default_source: Some("mic".into()),
            sinks: vec![sink(1, "speakers"), sink(2, "headset")],
            sources: vec![Device {
                monitor: None,
                ..sink(3, "mic")
            }],
            streams: vec![AppStream {
                index: 40,
                sink: 2,
                app: "Firefox".into(),
                title: String::new(),
                icon: None,
                volume: 100,
                muted: false,
                corked: false,
            }],
        };
        let mut taps = wanted(&snapshot);
        assert_eq!(taps.len(), 3);
        assert_eq!(taps["output"].source, "speakers.monitor");
        assert_eq!(taps["input"].source, "mic");
        assert_eq!(
            taps["40"],
            Tap {
                source: "headset.monitor".into(),
                stream: Some(40)
            }
        );

        // Nothing for a sleeping mic, or an app that paused.
        snapshot.sources[0].running = false;
        snapshot.streams[0].corked = true;
        taps = wanted(&snapshot);
        assert_eq!(taps.keys().collect::<Vec<_>>(), ["output"]);
        // Nor for an output that sleeps.
        snapshot.sinks[0].running = false;
        assert!(wanted(&snapshot).is_empty());
    }

    #[test]
    fn finds_the_loudest_sample() {
        let bytes: Vec<u8> = [0.1f32, -0.7, 0.3]
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        assert!((loudest(&bytes) - 0.7).abs() < 1e-6);
        assert!(loudest(&[]).abs() < f32::EPSILON);
    }
}

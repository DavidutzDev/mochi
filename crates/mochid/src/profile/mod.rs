//! The profiler: what mochid, Quickshell and the programs they start cost,
//! for the dev panel and `mochi profile`. Nothing runs until one of them
//! asks: then a thread reads the process tree twice a second, the UI counts
//! the frames each window draws, and a recording also samples mochid's own
//! stacks for a flame graph. Each module's time in its own task is counted
//! all the time, which costs two clock reads per poll.

mod report;
pub mod sampler;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use mochi_protocol::ActivityId;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

pub use report::Meta;
use sampler::Sample;

use crate::ipc::ConnectionId;

/// How often the tree is read.
pub const INTERVAL: Duration = Duration::from_millis(500);
/// How much the panel keeps: a minute.
const HISTORY: usize = 120;
/// The longest recording.
pub const MAX_SECONDS: u32 = 300;
/// How often mochid's stacks are sampled while recording, a prime so it
/// doesn't beat with timers.
const FLAME_HZ: i32 = 199;

/// Each module's time in its own task so far, in nanoseconds.
static MODULE_TIME: LazyLock<Mutex<BTreeMap<&'static str, Arc<AtomicU64>>>> =
    LazyLock::new(Mutex::default);

/// Counts the time `future`, a module's task, spends being polled.
pub fn timed<F: Future + Unpin>(module: &'static str, future: F) -> Timed<F> {
    let clock = MODULE_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry(module)
        .or_default()
        .clone();
    Timed { future, clock }
}

fn module_times() -> Vec<(&'static str, u64)> {
    MODULE_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|(module, clock)| (*module, clock.load(Ordering::Relaxed)))
        .collect()
}

/// Hands memory malloc keeps after it's freed back to the system, so a
/// burst like a report doesn't stay in mochid's memory.
fn give_back() {
    // SAFETY: malloc_trim only releases free memory malloc holds; it
    // takes no pointers.
    unsafe {
        libc::malloc_trim(0);
    }
}

/// A future whose polls are timed: see [`timed`].
#[derive(Debug)]
pub struct Timed<F> {
    future: F,
    clock: Arc<AtomicU64>,
}

impl<F: Future + Unpin> Future for Timed<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let this = self.get_mut();
        let start = Instant::now();
        let poll = Pin::new(&mut this.future).poll(cx);
        let spent = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        this.clock.fetch_add(spent, Ordering::Relaxed);
        poll
    }
}

/// Whose a process is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Mochid,
    Quickshell,
    /// What mochid or Quickshell started, like nvidia-smi or curl.
    Helpers,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Mochid, Group::Quickshell, Group::Helpers];

    fn of(pid: u32, comm: &str, root: u32) -> Self {
        if pid == root {
            Group::Mochid
        } else if comm.contains("quickshell") {
            Group::Quickshell
        } else {
            Group::Helpers
        }
    }
}

/// What the tree cost between two readings.
#[derive(Debug, Clone, Serialize)]
pub struct Point {
    /// Seconds since the profiler started.
    pub t: f64,
    /// CPU in percent of one core.
    pub cpu: BTreeMap<Group, f64>,
    /// Context switches per second.
    pub wakeups: BTreeMap<Group, f64>,
    /// Resident memory, in MB.
    pub memory: BTreeMap<Group, f64>,
    /// Each thread's CPU, in percent of one core, the busiest first.
    pub threads: Vec<ThreadUse>,
    /// Each module's time in its own task, in percent of one core.
    pub modules: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ThreadUse {
    pub group: Group,
    pub process: String,
    pub thread: String,
    pub cpu: f64,
}

/// What changed from `before` to `after`. A thread that wasn't there
/// before counts all its time, so a helper that came and went between
/// two readings still shows.
pub fn point(before: &Sample, after: &Sample, root: u32, origin: Instant) -> Point {
    let seconds = after
        .at
        .duration_since(before.at)
        .as_secs_f64()
        .max(f64::EPSILON);
    let percent = |nanos: u64| nanos as f64 / seconds / 1e7;
    let groups: HashMap<u32, (Group, &str)> = after
        .processes
        .iter()
        .map(|process| {
            let group = Group::of(process.pid, &process.comm, root);
            (process.pid, (group, process.comm.as_str()))
        })
        .collect();
    let earlier: HashMap<(u32, u32), u64> = before
        .threads
        .iter()
        .map(|thread| ((thread.pid, thread.tid), thread.cpu_ns))
        .collect();

    let mut cpu: BTreeMap<Group, f64> = Group::ALL.into_iter().map(|group| (group, 0.0)).collect();
    let mut threads = Vec::new();
    for thread in &after.threads {
        let Some((group, process)) = groups.get(&thread.pid) else {
            continue;
        };
        let spent = thread
            .cpu_ns
            .saturating_sub(earlier.get(&(thread.pid, thread.tid)).copied().unwrap_or(0));
        *cpu.entry(*group).or_default() += percent(spent);
        threads.push(ThreadUse {
            group: *group,
            process: (*process).to_owned(),
            thread: thread.name.clone(),
            cpu: percent(spent),
        });
    }
    threads.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));

    let switched: HashMap<u32, u64> = before
        .processes
        .iter()
        .map(|process| (process.pid, process.switches))
        .collect();
    let mut wakeups: BTreeMap<Group, f64> =
        Group::ALL.into_iter().map(|group| (group, 0.0)).collect();
    let mut memory = wakeups.clone();
    for process in &after.processes {
        let group = Group::of(process.pid, &process.comm, root);
        let woke = process.switches.saturating_sub(
            switched
                .get(&process.pid)
                .copied()
                .unwrap_or(process.switches),
        );
        *wakeups.entry(group).or_default() += woke as f64 / seconds;
        *memory.entry(group).or_default() += process.rss_kb as f64 / 1024.0;
    }

    let spent: HashMap<&str, u64> = before.modules.iter().copied().collect();
    let mut modules: Vec<(String, f64)> = after
        .modules
        .iter()
        .map(|(module, total)| {
            let since = total.saturating_sub(spent.get(module).copied().unwrap_or(0));
            ((*module).to_owned(), percent(since))
        })
        .collect();
    modules.sort_by(|a, b| b.1.total_cmp(&a.1));

    Point {
        t: after.at.duration_since(origin).as_secs_f64(),
        cpu,
        wakeups,
        memory,
        threads,
        modules,
    }
}

/// A window's frames in one second.
#[derive(Debug, Clone, Serialize)]
pub struct FramePoint {
    pub t: f64,
    pub output: String,
    pub surface: String,
    pub fps: u32,
}

/// mochid's stacks, sampled for the flame graph.
struct Flame(pprof::ProfilerGuard<'static>);

impl std::fmt::Debug for Flame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Flame")
    }
}

#[derive(Debug)]
struct Recording {
    started: Instant,
    until: Instant,
    points: Vec<Point>,
    frames: Vec<FramePoint>,
    flame: Option<Flame>,
    /// Why there's no flame graph, when there isn't.
    flame_error: Option<String>,
    /// Who waits for the report's path.
    waiting: Vec<ConnectionId>,
}

/// A finished recording: who asked, and the report's path or what failed.
#[derive(Debug)]
pub struct Finished {
    pub waiting: Vec<ConnectionId>,
    pub result: Result<PathBuf, String>,
}

#[derive(Debug)]
pub struct Profiler {
    root: u32,
    origin: Instant,
    samples: UnboundedSender<Sample>,
    /// The running sampler's stop flag.
    stop: Option<Arc<AtomicBool>>,
    last: Option<Sample>,
    history: VecDeque<Point>,
    /// Each window's frames in its last second, by monitor and surface.
    frames: BTreeMap<(String, String), u32>,
    recording: Option<Recording>,
    /// The dev panel, while it's open.
    pub panel: Option<ActivityId>,
    last_report: Option<PathBuf>,
}

impl Profiler {
    pub fn new(samples: UnboundedSender<Sample>) -> Self {
        Self {
            root: std::process::id(),
            origin: Instant::now(),
            samples,
            stop: None,
            last: None,
            history: VecDeque::new(),
            frames: BTreeMap::new(),
            recording: None,
            panel: None,
            last_report: None,
        }
    }

    /// Whether anything wants readings now.
    pub fn on(&self) -> bool {
        self.stop.is_some()
    }

    /// Starts the sampler when the panel or a recording wants it, and stops
    /// it when neither does. Some(on) when that changed.
    pub fn sync(&mut self) -> Option<bool> {
        let wanted = self.panel.is_some() || self.recording.is_some();
        match (wanted, self.stop.take()) {
            (true, None) => {
                let stop = Arc::new(AtomicBool::new(false));
                sampler::spawn(self.root, INTERVAL, self.samples.clone(), stop.clone());
                self.stop = Some(stop);
                Some(true)
            }
            (false, Some(stop)) => {
                stop.store(true, Ordering::Relaxed);
                self.last = None;
                self.history.clear();
                self.frames.clear();
                Some(false)
            }
            (_, stop) => {
                self.stop = stop;
                None
            }
        }
    }

    /// Takes a reading: the panel's history and the recording get what
    /// changed since the last one.
    pub fn on_sample(&mut self, sample: Sample) {
        if !self.on() {
            return;
        }
        if let Some(before) = self.last.take() {
            let point = point(&before, &sample, self.root, self.origin);
            if let Some(recording) = &mut self.recording {
                recording.points.push(point.clone());
            }
            self.history.push_back(point);
            while self.history.len() > HISTORY {
                self.history.pop_front();
            }
        }
        self.last = Some(sample);
    }

    pub fn on_frames(&mut self, output: String, surface: String, frames: u32) {
        if !self.on() {
            return;
        }
        if let Some(recording) = &mut self.recording {
            recording.frames.push(FramePoint {
                t: self.origin.elapsed().as_secs_f64(),
                output: output.clone(),
                surface: surface.clone(),
                fps: frames,
            });
        }
        self.frames.insert((output, surface), frames);
    }

    /// Starts recording for `seconds`; `who` gets the report's path.
    pub fn record(&mut self, seconds: u32, who: Option<ConnectionId>) -> Result<(), String> {
        if !(1..=MAX_SECONDS).contains(&seconds) {
            return Err(format!("record from 1 to {MAX_SECONDS} seconds"));
        }
        if let Some(recording) = &mut self.recording {
            recording.waiting.extend(who);
            return Ok(());
        }
        let (flame, flame_error) = match pprof::ProfilerGuardBuilder::default()
            .frequency(FLAME_HZ)
            .blocklist(&["libc", "libgcc", "pthread", "vdso", "ld-"])
            .build()
        {
            Ok(guard) => (Some(Flame(guard)), None),
            Err(error) => {
                tracing::warn!(%error, "can't sample mochid's stacks: the report has no flame graph");
                (None, Some(error.to_string()))
            }
        };
        let now = Instant::now();
        self.recording = Some(Recording {
            started: now,
            until: now + Duration::from_secs(seconds.into()),
            points: Vec::new(),
            frames: Vec::new(),
            flame,
            flame_error,
            waiting: who.into_iter().collect(),
        });
        tracing::info!(seconds, "profiling");
        Ok(())
    }

    /// When the recording ends.
    pub fn deadline(&self) -> Option<Instant> {
        self.recording.as_ref().map(|recording| recording.until)
    }

    /// Ends a recording that's due, and writes its report.
    pub fn finish(&mut self, now: Instant, meta: &Meta) -> Option<Finished> {
        if self.deadline().is_none_or(|until| until > now) {
            return None;
        }
        let recording = self.recording.take()?;
        let flame = match recording.flame {
            Some(Flame(guard)) => match guard.report().build() {
                Ok(report) => {
                    let mut svg = Vec::new();
                    match report.flamegraph(&mut svg) {
                        Ok(()) => Ok(svg),
                        Err(error) => Err(error.to_string()),
                    }
                }
                Err(error) => Err(error.to_string()),
            },
            None => Err(recording
                .flame_error
                .unwrap_or_else(|| "no flame graph".into())),
        };
        // pprof leaves SIGPROF ignored once it stops, and the programs
        // mochid starts afterwards would inherit that.
        // SAFETY: restores the default action; no handler runs.
        unsafe {
            libc::signal(libc::SIGPROF, libc::SIG_DFL);
        }
        // Naming the functions parsed mochid's symbols, and its debug
        // information in a debug build: a hundred MB or more, which
        // backtrace would keep for good.
        backtrace::clear_symbol_cache();
        give_back();
        let start = recording.started.duration_since(self.origin).as_secs_f64();
        let result = report::write(&report::Recorded {
            meta,
            start,
            seconds: now.duration_since(recording.started).as_secs_f64(),
            points: &recording.points,
            frames: &recording.frames,
            flame: flame.as_deref().map_err(String::as_str),
        });
        match &result {
            Ok(path) => {
                tracing::info!(path = %path.display(), "wrote a profile");
                self.last_report = Some(path.clone());
            }
            Err(error) => tracing::warn!(%error, "can't write the profile"),
        }
        Some(Finished {
            waiting: recording.waiting,
            result,
        })
    }

    /// The dev panel's payload: the latest reading, the last minute, the
    /// frames, and the recording.
    pub fn payload(&self) -> Value {
        let now = self.history.back();
        let history: Vec<&BTreeMap<Group, f64>> =
            self.history.iter().map(|point| &point.cpu).collect();
        let frames: Vec<Value> = self
            .frames
            .iter()
            .map(|((output, surface), fps)| json!({ "output": output, "surface": surface, "fps": fps }))
            .collect();
        json!({
            "cpu": now.map(|point| &point.cpu),
            "wakeups": now.map(|point| &point.wakeups),
            "memory": now.map(|point| &point.memory),
            "threads": now.map(|point| point.threads.iter().take(5).collect::<Vec<_>>()),
            "modules": now.map(|point| {
                point
                    .modules
                    .iter()
                    .filter(|(_, cpu)| *cpu > 0.0)
                    .take(5)
                    .map(|(module, cpu)| json!({ "module": module, "cpu": cpu }))
                    .collect::<Vec<_>>()
            }),
            "history": history,
            "frames": frames,
            "recording": self.recording.as_ref().map(|recording| {
                recording.until.saturating_duration_since(Instant::now()).as_secs()
            }),
            "report": self.last_report.as_ref().map(|path| path.display().to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::sampler::{Process, Thread};
    use super::*;

    fn sample(at: Instant, cpu: [u64; 3], switches: u64) -> Sample {
        Sample {
            at,
            processes: vec![
                Process {
                    pid: 10,
                    comm: ".mochid-wrapped".into(),
                    rss_kb: 51_200,
                    switches,
                },
                Process {
                    pid: 11,
                    comm: ".quickshell-wra".into(),
                    rss_kb: 102_400,
                    switches: switches * 10,
                },
                Process {
                    pid: 12,
                    comm: "nvidia-smi".into(),
                    rss_kb: 1024,
                    switches: 0,
                },
            ],
            threads: vec![
                Thread {
                    pid: 10,
                    tid: 10,
                    name: "tokio-rt-worker".into(),
                    cpu_ns: cpu[0],
                },
                Thread {
                    pid: 11,
                    tid: 11,
                    name: ".quickshell-wra".into(),
                    cpu_ns: cpu[1],
                },
                Thread {
                    pid: 12,
                    tid: 12,
                    name: "nvidia-smi".into(),
                    cpu_ns: cpu[2],
                },
            ],
            modules: vec![("media", cpu[0] / 2)],
        }
    }

    #[test]
    fn a_point_is_what_changed_in_percent_of_a_core() {
        let origin = Instant::now();
        let before = sample(origin, [1_000_000_000, 0, 0], 100);
        let after = sample(
            origin + Duration::from_millis(500),
            [1_005_000_000, 50_000_000, 0],
            110,
        );
        let point = point(&before, &after, 10, origin);
        assert!((point.t - 0.5).abs() < 1e-9);
        // 5 ms in half a second is 1% of a core; 50 ms is 10%.
        assert!((point.cpu[&Group::Mochid] - 1.0).abs() < 1e-6);
        assert!((point.cpu[&Group::Quickshell] - 10.0).abs() < 1e-6);
        assert_eq!(point.threads[0].process, ".quickshell-wra");
        // 10 and 100 switches in half a second.
        assert!((point.wakeups[&Group::Mochid] - 20.0).abs() < 1e-6);
        assert!((point.wakeups[&Group::Quickshell] - 200.0).abs() < 1e-6);
        assert!((point.memory[&Group::Quickshell] - 100.0).abs() < 1e-6);
        // The module had half of mochid's time.
        assert_eq!(point.modules[0].0, "media");
        assert!((point.modules[0].1 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_helper_new_since_the_last_reading_counts_all_its_time() {
        let origin = Instant::now();
        let mut before = sample(origin, [0, 0, 0], 0);
        before.threads.retain(|thread| thread.pid != 12);
        let after = sample(origin + Duration::from_secs(1), [0, 0, 20_000_000], 0);
        let point = point(&before, &after, 10, origin);
        assert!((point.cpu[&Group::Helpers] - 2.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn timed_futures_count_their_polls() {
        let module = "profile-test";
        let busy = timed(
            module,
            Box::pin(async {
                std::thread::sleep(Duration::from_millis(20));
            }),
        );
        busy.await;
        let spent = module_times()
            .into_iter()
            .find(|(name, _)| *name == module)
            .map(|(_, nanos)| nanos)
            .unwrap();
        assert!(spent >= 20_000_000, "{spent}");
    }
}

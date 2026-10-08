//! Performance: CPU, memory and GPU use, their temperatures, and disk and
//! network speeds.
//!
//! The hub has a Performance page with each reading, a graph of the last
//! two minutes and the busiest processes, which it can end. A reading that
//! stays over its
//! notice level shows a short notice on the island, naming the busiest
//! process; one that stays over its critical level puts a red bubble next
//! to the island until it comes down. A spike says nothing: a reading must
//! stay up for `sustain_seconds`.
//!
//! Settings in `config.toml`, all optional; 0 turns a level off:
//!
//! ```toml
//! [module.performance]
//! interval_ms = 2000
//! sustain_seconds = 10
//! cpu = { notice = 90, critical = 98 }
//! memory = { notice = 85, critical = 95 }
//! gpu = { notice = 95, critical = 0 }
//! temperature = { notice = 85, critical = 95 }   # °C, CPU and GPU
//! ```

mod model;
mod sample;
mod tour;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::model::{Change, Flow, History, Limits, Watch};
use crate::sample::{AmdGpu, CpuTimes, Gpu, Memory, Process};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

const NOTICE: Duration = Duration::from_millis(3500);
const CRITICAL_NOTICE: Duration = Duration::from_secs(6);
/// How long the page's process list keeps updating after it asked.
const DETAIL: Duration = Duration::from_secs(30);
/// How many processes the page lists.
const TOP: usize = 6;
/// The widget's graphs: its setting, the graph's name, and whether it
/// shows unless set.
const GRAPHS: [(&str, &str, bool); 5] = [
    ("cpu", "CPU", true),
    ("memory", "memory", true),
    ("gpu", "GPU", true),
    ("disk", "disk", false),
    ("network", "network", false),
];

/// What the page orders its processes by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Sort {
    #[default]
    Cpu,
    Memory,
    Disk,
}

impl Sort {
    const ALL: [Self; 3] = [Self::Cpu, Self::Memory, Self::Disk];

    fn name(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Disk => "disk",
        }
    }
}

/// One process for the page.
#[derive(Debug, Clone)]
struct Row {
    pid: u32,
    name: String,
    /// Percent of the whole machine since the last reading.
    cpu: f64,
    /// Resident memory, in KiB.
    memory: u64,
    /// Bytes a second read from and written to storage; `None` when
    /// `/proc/<pid>/io` isn't readable, as for other users' processes.
    disk: Option<f64>,
}

#[derive(Debug, Default)]
pub struct Performance;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    interval_ms: u64,
    sustain_seconds: u64,
    cpu: Limits,
    memory: Limits,
    gpu: Limits,
    temperature: Limits,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interval_ms: 2000,
            sustain_seconds: 10,
            cpu: Limits {
                notice: 90,
                critical: 98,
            },
            memory: Limits {
                notice: 85,
                critical: 95,
            },
            // A game keeps the GPU busy; only a long stretch at the top
            // says anything.
            gpu: Limits {
                notice: 95,
                critical: 0,
            },
            temperature: Limits {
                notice: 85,
                critical: 95,
            },
        }
    }
}

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if settings.interval_ms < 500 {
            return Err(format!(
                "interval_ms is {}; take at least 500 between readings",
                settings.interval_ms
            ));
        }
        Ok(settings)
    }
}

/// What a reading is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Metric {
    Cpu,
    Memory,
    Gpu,
    CpuTemperature,
    GpuTemperature,
}

impl Metric {
    const ALL: [Self; 5] = [
        Self::Cpu,
        Self::Memory,
        Self::Gpu,
        Self::CpuTemperature,
        Self::GpuTemperature,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Cpu | Self::CpuTemperature => "CPU",
            Self::Memory => "Memory",
            Self::Gpu | Self::GpuTemperature => "GPU",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Cpu => "chip",
            Self::Memory => "memory",
            Self::Gpu => "gpu",
            Self::CpuTemperature | Self::GpuTemperature => "temperature",
        }
    }

    fn temperature(self) -> bool {
        matches!(self, Self::CpuTemperature | Self::GpuTemperature)
    }

    fn limits(self, settings: &Settings) -> Limits {
        match self {
            Self::Cpu => settings.cpu,
            Self::Memory => settings.memory,
            Self::Gpu => settings.gpu,
            Self::CpuTemperature | Self::GpuTemperature => settings.temperature,
        }
    }

    fn value(self, value: f64) -> String {
        if self.temperature() {
            format!("{} °C", value.round())
        } else {
            format!("{}%", value.round())
        }
    }
}

impl Module for Performance {
    fn id(&self) -> &'static str {
        "performance"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        Settings::load(table).map(drop)
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("hub", "page", "page", "Page", "Performance")
                .icon("chip")
                .order(18),
            ContributionSpec::new("widgets", "widget", "graphs", "Widget", "Performance")
                .icon("chip")
                .options(json!({
                    "size": [18, 14],
                    "min": [12, 5],
                    "max": [50, 40],
                    "settings": GRAPHS.map(|(name, title, default)| json!({
                        "name": name,
                        "kind": "bool",
                        "default": default,
                        "description": format!("Show the {title} graph"),
                    })),
                })),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let pid = || ArgSpec::int("pid", "The process, one of yours");
        vec![
            ActionSpec::new("status", "Print the readings now"),
            ActionSpec::new(
                "detail",
                "List the busiest processes for a while; the page sends this",
            ),
            ActionSpec::new("sort", "Order the page's processes").arg(ArgSpec::choice(
                "by",
                "What they use most of",
                Sort::ALL.map(Sort::name),
            )),
            ActionSpec::new("end", "Ask a process to quit, with SIGTERM").arg(pid()),
            ActionSpec::new("kill", "Stop a process at once, with SIGKILL").arg(pid()),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused a too short interval.
            let settings: Settings = ctx.settings()?;
            let interval = Duration::from_millis(settings.interval_ms);
            let mut state = State::new(settings);
            let mut ticks = tokio::time::interval(interval);
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::BubbleClicked(_)) => {
                            let open = ctx.call("hub", "open", &["performance/page"]);
                            tokio::spawn(async move {
                                if let Err(error) = open.await {
                                    tracing::debug!(%error, "can't open the hub");
                                }
                            });
                        }
                        Some(_) => {}
                    },
                    _ = ticks.tick() => state.sample(&ctx),
                }
            }
        })
    }
}

/// One reading of everything.
#[derive(Debug, Clone, Default)]
struct Reading {
    cpu: f64,
    cpu_temperature: Option<f64>,
    memory: Memory,
    gpu: Option<Gpu>,
}

impl Reading {
    fn value(&self, metric: Metric) -> Option<f64> {
        match metric {
            Metric::Cpu => Some(self.cpu),
            Metric::Memory => Some(self.memory.percent()),
            Metric::Gpu => self.gpu.as_ref().map(|gpu| gpu.usage),
            Metric::CpuTemperature => self.cpu_temperature,
            Metric::GpuTemperature => self.gpu.as_ref().and_then(|gpu| gpu.temperature),
        }
    }
}

#[derive(Debug)]
struct State {
    settings: Settings,
    cores: usize,
    cpu_before: Option<CpuTimes>,
    cpu_sensor: Option<std::path::PathBuf>,
    amd: Option<AmdGpu>,
    nvidia: Option<watch::Receiver<Option<Gpu>>>,
    reading: Reading,
    watches: HashMap<Metric, Watch>,
    histories: HashMap<Metric, History>,
    /// Reads and writes on the disks, and bytes in and out on the cards.
    disk: Flow,
    network: Flow,
    /// Each process's CPU ticks at the last reading, and the machine's.
    process_ticks: HashMap<u32, u64>,
    total_before: u64,
    /// Each process's storage bytes at the last reading, and when that was.
    process_io: HashMap<u32, u64>,
    io_before: Option<Instant>,
    /// Every process at the last reading.
    rows: Vec<Row>,
    /// The few the page shows, in `sort` order, with their full names.
    top: Vec<Row>,
    sort: Sort,
    /// The user the daemon runs as, whose processes the page can end.
    uid: Option<u32>,
    detail_until: Option<Instant>,
    bubble: Option<BubbleId>,
    /// The critical readings the bubble shows, to tell news from new values.
    critical_shown: Vec<Value>,
}

impl State {
    fn new(settings: Settings) -> Self {
        let amd = AmdGpu::find();
        let nvidia = amd
            .is_none()
            .then(|| sample::follow_nvidia(settings.interval_ms))
            .flatten();
        let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
        let mut state = Self {
            cores: sample::cores(&stat),
            cpu_before: sample::cpu_times(&stat),
            cpu_sensor: sample::cpu_sensor(),
            amd,
            nvidia,
            settings,
            reading: Reading::default(),
            watches: HashMap::new(),
            histories: HashMap::new(),
            disk: Flow::default(),
            network: Flow::default(),
            process_ticks: HashMap::new(),
            total_before: 0,
            process_io: HashMap::new(),
            io_before: None,
            rows: Vec::new(),
            top: Vec::new(),
            sort: Sort::default(),
            uid: sample::owner("self"),
            detail_until: None,
            bubble: None,
            critical_shown: Vec::new(),
        };
        // The first reading then has counts to compare with.
        state.traffic(Instant::now());
        state
    }

    /// Reads the disks' and the cards' byte counts.
    fn traffic(&mut self, now: Instant) {
        let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
        self.disk.update(
            sample::disk_bytes(&read("/proc/diskstats"), sample::is_disk),
            now,
        );
        self.network.update(
            sample::net_bytes(&read("/proc/net/dev"), sample::is_card),
            now,
        );
    }

    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let action = command.action.clone();
        match action.as_str() {
            "status" => {
                let lines: Vec<String> = Metric::ALL
                    .into_iter()
                    .filter_map(|metric| {
                        let value = self.reading.value(metric)?;
                        let kind = if metric.temperature() {
                            "temperature"
                        } else {
                            "use"
                        };
                        Some(format!(
                            "{} {kind}\t{}",
                            metric.label(),
                            metric.value(value)
                        ))
                    })
                    .collect();
                let (read, written) = self.disk.speeds;
                let (down, up) = self.network.speeds;
                let lines = [
                    lines.join("\n"),
                    format!("Disk read\t{}\nDisk write\t{}", speed(read), speed(written)),
                    format!("Download\t{}\nUpload\t{}", speed(down), speed(up)),
                ];
                command.answer(Ok(lines.join("\n")));
            }
            "detail" => {
                let first = self.detail_until.is_none();
                self.detail_until = Some(Instant::now() + DETAIL);
                if first {
                    // A list at once, rather than at the next reading.
                    self.processes();
                    self.publish(ctx);
                }
                command.reply(Ok(()));
            }
            "sort" => {
                let by = command.args.str("by").unwrap_or_default();
                self.sort = Sort::ALL
                    .into_iter()
                    .find(|sort| sort.name() == by)
                    .unwrap_or_default();
                self.rank();
                self.publish(ctx);
                command.reply(Ok(()));
            }
            "end" | "kill" => {
                let pid = command.args.int("pid").unwrap_or_default();
                let signal = if action == "end" {
                    libc::SIGTERM
                } else {
                    libc::SIGKILL
                };
                let result = sample::signal(pid, signal);
                match &result {
                    Ok(()) => tracing::info!(pid, action, "signalled a process"),
                    Err(error) => tracing::info!(pid, action, %error, "can't signal a process"),
                }
                command.reply(result);
            }
            other => command.reply(Err(format!("performance has no action {other}"))),
        }
    }

    fn sample(&mut self, ctx: &ModuleCtx) {
        let now = Instant::now();
        let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
        let times = sample::cpu_times(&stat);
        if let (Some(before), Some(after)) = (self.cpu_before, times) {
            self.reading.cpu = sample::usage(before, after);
        }
        self.cpu_before = times;
        self.reading.memory =
            sample::memory(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default());
        self.reading.cpu_temperature = self.cpu_sensor.as_deref().and_then(sample::temperature);
        self.reading.gpu = match (&self.amd, &self.nvidia) {
            (Some(amd), _) => amd.read(),
            (None, Some(nvidia)) => nvidia.borrow().clone(),
            (None, None) => None,
        };
        self.traffic(now);

        if self.detail_until.is_some_and(|until| until < now) {
            self.detail_until = None;
        }
        // Processes cost a pass over /proc: only for the page, or when a
        // notice may need the busiest one.
        let busy = self.reading.cpu >= f64::from(self.settings.cpu.notice.max(1)) - 10.0
            || self.reading.memory.percent()
                >= f64::from(self.settings.memory.notice.max(1)) - 10.0;
        if self.detail_until.is_some() || busy {
            self.processes();
        } else {
            self.process_ticks.clear();
            self.process_io.clear();
            self.io_before = None;
            self.rows.clear();
            self.top.clear();
        }

        let sustain = Duration::from_secs(self.settings.sustain_seconds);
        for metric in Metric::ALL {
            let Some(value) = self.reading.value(metric) else {
                continue;
            };
            self.histories.entry(metric).or_default().push(value);
            let limits = metric.limits(&self.settings);
            let change = self
                .watches
                .entry(metric)
                .or_default()
                .update(value, limits, sustain, now);
            match change {
                Some(Change::High) => self.notice(ctx, metric, value, false),
                Some(Change::Critical) => self.notice(ctx, metric, value, true),
                Some(Change::Calm) | None => {}
            }
        }
        self.publish(ctx);
    }

    /// Reads every process: its CPU since the last reading, its memory,
    /// and, while the page is open, its disk use.
    fn processes(&mut self) {
        let now = Instant::now();
        let total = self.cpu_before.map_or(0, |times| times.total);
        let elapsed = total.saturating_sub(self.total_before);
        self.total_before = total;
        // Disk use costs one more file per process, so only for the page.
        let io_seconds = self.detail_until.is_some().then(|| {
            self.io_before
                .replace(now)
                .map_or(0.0, |then| now.duration_since(then).as_secs_f64())
        });
        let processes: Vec<Process> = sample::processes();
        let mut ticks = HashMap::with_capacity(processes.len());
        let mut io = HashMap::new();
        self.rows = processes
            .into_iter()
            .map(|process| {
                let before = self.process_ticks.get(&process.pid).copied();
                ticks.insert(process.pid, process.ticks);
                let cpu = match before {
                    // The share of the whole machine, like the CPU reading.
                    Some(before) if elapsed > 0 => {
                        process.ticks.saturating_sub(before) as f64 / elapsed as f64 * 100.0
                    }
                    _ => 0.0,
                };
                let disk = io_seconds.and_then(|seconds| {
                    let bytes = std::fs::read_to_string(format!("/proc/{}/io", process.pid))
                        .ok()
                        .as_deref()
                        .and_then(sample::process_io)?;
                    let before = self.process_io.get(&process.pid).copied();
                    io.insert(process.pid, bytes);
                    Some(match before {
                        Some(before) if seconds > 0.0 => {
                            bytes.saturating_sub(before) as f64 / seconds
                        }
                        _ => 0.0,
                    })
                });
                Row {
                    pid: process.pid,
                    name: process.name,
                    cpu,
                    memory: process.memory,
                    disk,
                }
            })
            .collect();
        self.process_ticks = ticks;
        self.process_io = io;
        self.rank();
    }

    /// Picks the page's processes from the last reading, in `sort` order.
    fn rank(&mut self) {
        let mut rows: Vec<&Row> = self.rows.iter().collect();
        let by_cpu = |a: &Row, b: &Row| b.cpu.total_cmp(&a.cpu).then(b.memory.cmp(&a.memory));
        match self.sort {
            Sort::Cpu => rows.sort_by(|a, b| by_cpu(a, b)),
            Sort::Memory => rows.sort_by_key(|row| std::cmp::Reverse(row.memory)),
            Sort::Disk => rows.sort_by(|a, b| {
                let disk = |row: &Row| row.disk.unwrap_or(-1.0);
                disk(b).total_cmp(&disk(a)).then_with(|| by_cpu(a, b))
            }),
        }
        // Full names only for the few shown: each costs a file read.
        self.top = rows
            .into_iter()
            .take(TOP)
            .map(|row| Row {
                name: sample::display_name(row.pid, &row.name),
                ..row.clone()
            })
            .collect();
    }

    /// The process using the most of a reading, for a notice.
    fn culprit(&self, metric: Metric) -> Option<String> {
        match metric {
            Metric::Cpu | Metric::CpuTemperature => self
                .rows
                .iter()
                .max_by(|a, b| a.cpu.total_cmp(&b.cpu))
                .filter(|row| row.cpu >= 1.0)
                .map(|row| sample::display_name(row.pid, &row.name)),
            Metric::Memory => {
                let mut processes = sample::processes();
                processes.sort_by_key(|process| std::cmp::Reverse(process.memory));
                processes.first().map(|process| {
                    let name = sample::display_name(process.pid, &process.name);
                    format!("{name} uses {}", gigabytes(process.memory))
                })
            }
            Metric::Gpu | Metric::GpuTemperature => None,
        }
    }

    fn notice(&self, ctx: &ModuleCtx, metric: Metric, value: f64, critical: bool) {
        let what = if metric.temperature() {
            format!("{} at {}", metric.label(), metric.value(value))
        } else if critical {
            format!("{} critical · {}", metric.label(), metric.value(value))
        } else {
            format!("{} at {}", metric.label(), metric.value(value))
        };
        let text = match self.culprit(metric) {
            Some(culprit) => format!("{what} · {culprit}"),
            None => what,
        };
        tracing::info!(%text, "performance");
        let spec = ActivitySpec::new("Notice")
            .key("notice")
            .payload(json!({ "icon": metric.icon(), "text": text, "critical": critical }));
        let spec = if critical {
            spec.priority(Priority::HIGH).timeout(CRITICAL_NOTICE)
        } else {
            spec.priority(Priority::HIGH)
                .passive()
                .fleeting()
                .timeout(NOTICE)
        };
        ctx.present(spec);
    }

    fn publish(&mut self, ctx: &ModuleCtx) {
        let history = |metric: Metric| {
            self.histories
                .get(&metric)
                .map(History::values)
                .unwrap_or_default()
        };
        let memory = &self.reading.memory;
        let gpu = self.reading.gpu.as_ref().map(|gpu| {
            json!({
                "name": gpu.name,
                "usage": gpu.usage.round(),
                "memory_used": gpu.memory_used,
                "memory_total": gpu.memory_total,
                "temperature": gpu.temperature.map(f64::round),
                "history": history(Metric::Gpu),
            })
        });
        let critical: Vec<Value> = Metric::ALL
            .into_iter()
            .filter(|metric| self.watches.get(metric).is_some_and(|watch| watch.critical))
            .filter_map(|metric| {
                let value = self.reading.value(metric)?;
                Some(json!({ "icon": metric.icon(), "label": metric.label(), "value": metric.value(value) }))
            })
            .collect();
        ctx.publish_state(json!({
            "cpu": {
                "usage": self.reading.cpu.round(),
                "cores": self.cores,
                "temperature": self.reading.cpu_temperature.map(f64::round),
                "history": history(Metric::Cpu),
            },
            "memory": {
                "percent": memory.percent().round(),
                "used": gigabytes(memory.used),
                "total": gigabytes(memory.total),
                "swap_used": gigabytes(memory.swap_used),
                "swap_total": gigabytes(memory.swap_total),
                "history": history(Metric::Memory),
            },
            "gpu": gpu,
            "disk": flow(&self.disk),
            "network": flow(&self.network),
            "sort": self.sort.name(),
            "processes": self.top.iter().map(|row| json!({
                "pid": row.pid,
                "name": row.name,
                "cpu": (row.cpu * 10.0).round() / 10.0,
                "memory": gigabytes(row.memory),
                "disk": row.disk.map(speed),
                "own": self.uid.is_some() && sample::owner(&row.pid.to_string()) == self.uid,
            })).collect::<Vec<_>>(),
            "critical": critical,
        }));

        if critical.is_empty() {
            if let Some(bubble) = self.bubble.take() {
                ctx.hide_bubble(bubble);
            }
        } else {
            // Another reading turning critical is news; new values aren't.
            let labels: Vec<Value> = critical.iter().map(|entry| entry["icon"].clone()).collect();
            let news = self.bubble.is_some() && labels != self.critical_shown;
            self.critical_shown = labels;
            let spec = BubbleSpec::new("Bubble")
                .key("critical")
                .area(Area::Right)
                .group("status")
                .payload(json!({ "critical": critical }));
            let spec = if news { spec.news() } else { spec };
            self.bubble = Some(ctx.show_bubble(spec));
        }
    }
}

/// A disk's or a card's two speeds for the page: `in` is read or
/// received, `out` written or sent; histories in bytes a second.
fn flow(flow: &Flow) -> Value {
    json!({
        "in": speed(flow.speeds.0),
        "out": speed(flow.speeds.1),
        "in_history": flow.histories.0.values(),
        "out_history": flow.histories.1.values(),
    })
}

/// `512 B/s`, `3.4 KB/s`, `120 MB/s`, from bytes a second.
fn speed(bytes: f64) -> String {
    let mut value = bytes.max(0.0);
    let mut unit = 0;
    let units = ["B/s", "KB/s", "MB/s", "GB/s"];
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value >= 100.0 {
        format!("{} {}", value.round(), units[unit])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

/// `3.1 GB`, `512 MB`, from kibibytes.
fn gigabytes(kibibytes: u64) -> String {
    let megabytes = kibibytes as f64 / 1024.0;
    if megabytes < 1024.0 {
        format!("{} MB", megabytes.round())
    } else {
        format!("{:.1} GB", megabytes / 1024.0)
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "performance",
            include_str!("../settings.toml"),
        );
    }

    #[test]
    fn checks_settings() {
        let table = |text: &str| mochi_core::toml::from_str(text).unwrap();
        assert!(super::Settings::load(&table("cpu = { notice = 70 }")).is_ok());
        assert!(super::Settings::load(&table("interval_ms = 100")).is_err());
        assert!(super::Settings::load(&table("cpu = { warning = 70 }")).is_err());
        assert_eq!(super::gigabytes(512 * 1024), "512 MB");
        assert_eq!(super::gigabytes(3 * 1024 * 1024 + 100 * 1024), "3.1 GB");
    }

    #[test]
    fn scales_speeds() {
        assert_eq!(super::speed(0.0), "0 B/s");
        assert_eq!(super::speed(512.4), "512 B/s");
        assert_eq!(super::speed(3.4 * 1024.0), "3.4 KB/s");
        assert_eq!(super::speed(120.0 * 1024.0 * 1024.0), "120 MB/s");
        assert_eq!(super::speed(1.2 * 1024.0 * 1024.0 * 1024.0), "1.2 GB/s");
    }
}

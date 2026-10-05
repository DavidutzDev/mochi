//! Reading the machine: CPU use and temperature from `/proc` and hwmon,
//! memory from `/proc/meminfo`, the GPU from AMD's sysfs or NVIDIA's
//! `nvidia-smi`, and the busiest processes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::watch;

/// CPU time so far, from the first line of `/proc/stat`, in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CpuTimes {
    pub busy: u64,
    pub total: u64,
}

pub fn cpu_times(stat: &str) -> Option<CpuTimes> {
    let fields: Vec<u64> = stat
        .lines()
        .next()?
        .strip_prefix("cpu ")?
        .split_whitespace()
        .filter_map(|field| field.parse().ok())
        .collect();
    // user nice system idle iowait irq softirq steal; guests are in user.
    let total: u64 = fields.iter().take(8).sum();
    let idle = fields.get(3)? + fields.get(4).unwrap_or(&0);
    Some(CpuTimes {
        busy: total.saturating_sub(idle),
        total,
    })
}

/// Percent busy between two readings.
pub fn usage(before: CpuTimes, after: CpuTimes) -> f64 {
    let total = after.total.saturating_sub(before.total);
    if total == 0 {
        return 0.0;
    }
    let busy = after.busy.saturating_sub(before.busy);
    (busy as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
}

/// How many CPUs `/proc/stat` lists.
pub fn cores(stat: &str) -> usize {
    stat.lines()
        .filter(|line| line.starts_with("cpu") && !line.starts_with("cpu "))
        .count()
        .max(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Memory {
    /// In kibibytes.
    pub total: u64,
    pub used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

impl Memory {
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.used as f64 / self.total as f64 * 100.0
    }
}

pub fn memory(meminfo: &str) -> Memory {
    let fields: HashMap<&str, u64> = meminfo
        .lines()
        .filter_map(|line| {
            let (key, rest) = line.split_once(':')?;
            Some((key, rest.split_whitespace().next()?.parse().ok()?))
        })
        .collect();
    let get = |key: &str| fields.get(key).copied().unwrap_or_default();
    Memory {
        total: get("MemTotal"),
        used: get("MemTotal").saturating_sub(get("MemAvailable")),
        swap_total: get("SwapTotal"),
        swap_used: get("SwapTotal").saturating_sub(get("SwapFree")),
    }
}

/// The CPU's temperature sensor: AMD's k10temp or zenpower, Intel's
/// coretemp, or an ARM board's.
pub fn cpu_sensor() -> Option<PathBuf> {
    let mut found = None;
    for entry in std::fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        let dir = entry.path();
        let name = std::fs::read_to_string(dir.join("name")).unwrap_or_default();
        if matches!(
            name.trim(),
            "k10temp" | "zenpower" | "coretemp" | "cpu_thermal"
        ) && dir.join("temp1_input").exists()
        {
            found = Some(dir.join("temp1_input"));
            break;
        }
    }
    found
}

/// Degrees Celsius from a hwmon file, which counts millidegrees.
pub fn temperature(path: &Path) -> Option<f64> {
    let millidegrees: f64 = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
    Some(millidegrees / 1000.0)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Gpu {
    pub name: String,
    pub usage: f64,
    /// In mebibytes.
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: Option<f64>,
}

/// An AMD GPU, from its sysfs files.
#[derive(Debug, Clone)]
pub struct AmdGpu {
    device: PathBuf,
    sensor: Option<PathBuf>,
}

impl AmdGpu {
    pub fn find() -> Option<Self> {
        for entry in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
            let device = entry.path().join("device");
            if device.join("gpu_busy_percent").exists() {
                let sensor = std::fs::read_dir(device.join("hwmon"))
                    .ok()
                    .and_then(|mut dirs| dirs.next())
                    .and_then(Result::ok)
                    .map(|dir| dir.path().join("temp1_input"));
                return Some(Self { device, sensor });
            }
        }
        None
    }

    pub fn read(&self) -> Option<Gpu> {
        let number = |file: &str| -> Option<u64> {
            std::fs::read_to_string(self.device.join(file))
                .ok()?
                .trim()
                .parse()
                .ok()
        };
        Some(Gpu {
            name: "AMD GPU".into(),
            usage: number("gpu_busy_percent")? as f64,
            memory_used: number("mem_info_vram_used").unwrap_or_default() >> 20,
            memory_total: number("mem_info_vram_total").unwrap_or_default() >> 20,
            temperature: self.sensor.as_deref().and_then(temperature),
        })
    }
}

/// Follows an NVIDIA GPU through one `nvidia-smi` that prints a line every
/// `interval_ms`, rather than starting one each time. The latest reading
/// goes to the receiver.
pub fn follow_nvidia(interval_ms: u64) -> Option<watch::Receiver<Option<Gpu>>> {
    let mut child = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
            &format!("--loop-ms={interval_ms}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let (sender, receiver) = watch::channel(None);
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // The first GPU's line; more GPUs print one each.
            if let Some(gpu) = nvidia_line(&line)
                && sender.send(Some(gpu)).is_err()
            {
                break;
            }
        }
        let _ = child.kill().await;
    });
    Some(receiver)
}

pub fn nvidia_line(line: &str) -> Option<Gpu> {
    let fields: Vec<&str> = line.split(',').map(str::trim).collect();
    let [name, usage, used, total, temperature] = fields.as_slice() else {
        return None;
    };
    Some(Gpu {
        name: (*name).to_owned(),
        usage: usage.parse().ok()?,
        memory_used: used.parse().unwrap_or_default(),
        memory_total: total.parse().unwrap_or_default(),
        temperature: temperature.parse().ok(),
    })
}

/// A process's CPU time and memory, from `/proc/<pid>/stat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    /// User and system time, in ticks.
    pub ticks: u64,
    /// Resident memory, in kibibytes.
    pub memory: u64,
}

pub fn process(pid: u32, stat: &str) -> Option<Process> {
    // The name is in parentheses and may hold spaces or parentheses.
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat.get(open + 1..close)?.to_owned();
    let fields: Vec<&str> = stat.get(close + 2..)?.split_whitespace().collect();
    // After the name: state is field 3, utime 14, stime 15, rss 24.
    let field = |number: usize| -> Option<u64> { fields.get(number - 3)?.parse().ok() };
    Some(Process {
        pid,
        name,
        ticks: field(14)? + field(15)?,
        memory: field(24)? * 4,
    })
}

/// A process's name as people know it: the program it runs, from its
/// command line, since the kernel's name stops at 15 characters, without
/// the dot and `-wrapped` that Nix's wrappers add.
pub fn display_name(pid: u32, short: &str) -> String {
    let program = std::fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .and_then(|cmdline| {
            let first = cmdline.split(|byte| *byte == 0).next()?;
            let first = String::from_utf8_lossy(first).into_owned();
            // Electron apps rewrite theirs into one line, arguments and all.
            let program = first.split(' ').next()?;
            let name = program.rsplit('/').next()?.to_owned();
            (!name.is_empty()).then_some(name)
        });
    clean(program.as_deref().unwrap_or(short))
}

fn clean(name: &str) -> String {
    let name = name.strip_prefix('.').unwrap_or(name);
    let name = name.strip_suffix("-wrapped").unwrap_or(name);
    name.to_owned()
}

/// Every process now.
pub fn processes() -> Vec<Process> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(entry.path().join("stat")).ok()?;
            process(pid, &stat)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cpu_use() {
        let before =
            cpu_times("cpu  100 0 100 700 100 0 0 0 0 0\ncpu0 1 2 3 4\ncpu1 1 2 3 4\n").unwrap();
        let after = cpu_times("cpu  200 0 200 800 100 0 0 0 0 0\n").unwrap();
        assert_eq!(usage(before, after), 200.0 / 300.0 * 100.0);
        assert_eq!(cores("cpu  1\ncpu0 1\ncpu1 1\nintr 4\n"), 2);
        assert_eq!(usage(after, after), 0.0);
    }

    #[test]
    fn reads_memory() {
        let memory =
            memory("MemTotal: 1000 kB\nMemAvailable: 250 kB\nSwapTotal: 100 kB\nSwapFree: 40 kB\n");
        assert_eq!(memory.used, 750);
        assert_eq!(memory.percent(), 75.0);
        assert_eq!(memory.swap_used, 60);
    }

    #[test]
    fn reads_nvidia_smi() {
        let gpu = nvidia_line("NVIDIA GeForce GTX 1060 6GB, 33, 2484, 6144, 58").unwrap();
        assert_eq!(gpu.name, "NVIDIA GeForce GTX 1060 6GB");
        assert_eq!(gpu.usage, 33.0);
        assert_eq!(gpu.memory_total, 6144);
        assert_eq!(gpu.temperature, Some(58.0));
        assert!(nvidia_line("garbage").is_none());
    }

    #[test]
    fn cleans_wrapper_names() {
        assert_eq!(clean(".Discord-wrapped"), "Discord");
        assert_eq!(clean("firefox"), "firefox");
        assert_eq!(display_name(u32::MAX, ".kitty-wrapped"), "kitty");
    }

    #[test]
    fn reads_a_process() {
        let stat =
            "4242 (Web Content (x)) S 1 2 3 4 5 6 7 8 9 10 150 50 0 0 20 0 30 0 100 2000 512 0";
        let process = process(4242, stat).unwrap();
        assert_eq!(process.name, "Web Content (x)");
        assert_eq!(process.ticks, 200);
        assert_eq!(process.memory, 2048);
    }
}

//! Reading the machine: CPU use and temperature from `/proc` and hwmon,
//! memory from `/proc/meminfo`, the GPU from AMD's sysfs or NVIDIA's
//! `nvidia-smi`, disk and network traffic from `/proc/diskstats` and
//! `/proc/net/dev`, and the busiest processes.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
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

/// How full a filesystem is, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Space {
    pub used: u64,
    /// What a user can still write, without the part kept for root.
    pub free: u64,
}

impl Space {
    /// The share used of what users can have, as `df` counts it.
    pub fn percent(&self) -> f64 {
        let usable = self.used + self.free;
        if usable == 0 {
            return 0.0;
        }
        self.used as f64 / usable as f64 * 100.0
    }

    pub fn total(&self) -> u64 {
        self.used + self.free
    }
}

/// The filesystem `path` is on, from statvfs.
// Its fields are 32 bits wide on some systems.
#[allow(clippy::useless_conversion)]
pub fn space(path: &Path) -> Option<Space> {
    use std::os::unix::ffi::OsStrExt;

    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: the struct is plain integers, for which zero is a value.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: statvfs only writes into the struct it's given, and reads
    // the path, a NUL-terminated string that lives through the call.
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let unit = u64::from(stat.f_frsize);
    Some(Space {
        used: u64::from(stat.f_blocks).saturating_sub(u64::from(stat.f_bfree)) * unit,
        free: u64::from(stat.f_bavail) * unit,
    })
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

/// Bytes read from and written to the disks so far, from
/// `/proc/diskstats`. `keep` picks the disks by name, so partitions and
/// the devices stacked on a disk aren't counted twice.
pub fn disk_bytes(diskstats: &str, keep: impl Fn(&str) -> bool) -> (u64, u64) {
    let mut totals = (0, 0);
    for line in diskstats.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // major minor name, then reads, merged, sectors read, time, writes,
        // merged, sectors written.
        let (Some(name), Some(read), Some(written)) = (fields.get(2), fields.get(5), fields.get(9))
        else {
            continue;
        };
        if !keep(name) {
            continue;
        }
        // The kernel counts 512-byte sectors here, whatever the disk's.
        totals.0 += read.parse::<u64>().unwrap_or_default() * 512;
        totals.1 += written.parse::<u64>().unwrap_or_default() * 512;
    }
    totals
}

/// A whole disk on real hardware: the kernel lists those in `/sys/block`
/// with a `device` link. Partitions aren't in `/sys/block`, and loop,
/// RAM, zram, device-mapper and RAID devices have no `device`.
pub fn is_disk(name: &str) -> bool {
    Path::new("/sys/block").join(name).join("device").exists()
}

/// Bytes received and sent so far, from `/proc/net/dev`. `keep` picks the
/// interfaces by name.
pub fn net_bytes(dev: &str, keep: impl Fn(&str) -> bool) -> (u64, u64) {
    let mut totals = (0, 0);
    // Two header lines, then `name: received ... sent ...`.
    for line in dev.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        if !keep(name.trim()) {
            continue;
        }
        let fields: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|field| field.parse().ok())
            .collect();
        // Eight receive fields, bytes first, then eight transmit fields.
        if let (Some(received), Some(sent)) = (fields.first(), fields.get(8)) {
            totals.0 += received;
            totals.1 += sent;
        }
    }
    totals
}

/// A network card, wired or wireless: one with a `device` link in
/// `/sys/class/net`. Loopback, bridges, containers' veths and VPN tunnels
/// have none, and a tunnel's traffic crosses a card anyway.
pub fn is_card(name: &str) -> bool {
    Path::new("/sys/class/net")
        .join(name)
        .join("device")
        .exists()
}

/// The bytes a process read from and wrote to storage so far, from
/// `/proc/<pid>/io`. Only one's own processes are readable.
pub fn process_io(io: &str) -> Option<u64> {
    let field = |key: &str| -> Option<u64> {
        io.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))?
            .trim()
            .parse()
            .ok()
    };
    Some(field("read_bytes")? + field("write_bytes")?)
}

/// The user a process runs as; `self` for the daemon's own.
pub fn owner(pid: &str) -> Option<u32> {
    std::fs::metadata(format!("/proc/{pid}"))
        .ok()
        .map(|metadata| metadata.uid())
}

/// Sends a signal to one of this user's processes, never to the daemon
/// itself or to anyone else's.
pub fn signal(pid: i64, signal: libc::c_int) -> Result<(), String> {
    // 0 and below name groups of processes, or all of them, to kill().
    let target = libc::pid_t::try_from(pid)
        .ok()
        .filter(|target| *target > 0)
        .ok_or_else(|| format!("{pid} isn't a process"))?;
    if pid == i64::from(std::process::id()) {
        return Err("that's mochid itself".into());
    }
    match (owner(&pid.to_string()), owner("self")) {
        (None, _) => return Err(format!("no process {pid}")),
        (Some(theirs), Some(ours)) if theirs == ours => {}
        _ => return Err(format!("process {pid} belongs to another user")),
    }
    // SAFETY: `kill` takes plain integers, and the pid is above 0, so it
    // names one process rather than a group.
    if unsafe { libc::kill(target, signal) } == 0 {
        Ok(())
    } else {
        Err(format!(
            "can't signal {pid}: {}",
            std::io::Error::last_os_error()
        ))
    }
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
    fn counts_space_like_df() {
        // What's kept for root counts neither as used nor as free.
        let three_quarters = Space {
            used: 300,
            free: 100,
        };
        assert_eq!(three_quarters.percent(), 75.0);
        assert_eq!(three_quarters.total(), 400);
        assert_eq!(Space::default().percent(), 0.0);
        let root = space(Path::new("/")).unwrap();
        assert!(root.total() > 0);
        assert!(space(Path::new("/no/such/place")).is_none());
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

    const DISKSTATS: &str = "\
 259       0 nvme0n1 82877 23542 8322758 40563 107758 7203 3765212 667885 0 42272 718174 10845 0 6836288 3661 1725 6063
 259       1 nvme0n1p1 189 8132 36878 73 14 0 12 1 0 51 75 0 0 0 0 0 0
 253       0 zram0 95 0 3008 0 687 0 5496 12 0 12 12 0 0 0 0 0 0
   7       0 loop0 10 0 80 0 0 0 0 0 0 0 0 0 0 0 0 0 0
   8       0 sda 85 0 4776 25 3 0 24 0 0 11 25 0 0 0 0 0 0
   8       1 sda1 21 0 1544 8 0 0 0 0 0 8 8 0 0 0 0 0 0
 254       0 dm-0 500 0 9000 0 400 0 7000 0 0 0 0 0 0 0 0 0 0
";

    #[test]
    fn reads_the_disks() {
        let disks = |name: &str| matches!(name, "nvme0n1" | "sda");
        let (read, written) = disk_bytes(DISKSTATS, disks);
        assert_eq!(read, (8_322_758 + 4776) * 512);
        assert_eq!(written, (3_765_212 + 24) * 512);
        assert_eq!(disk_bytes(DISKSTATS, |_| false), (0, 0));
        assert_eq!(disk_bytes("garbage\n\n", |_| true), (0, 0));
    }

    const NET_DEV: &str = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 3848075    8735    0    0    0     0          0         0  3848075    8735    0    0    0     0       0          0
enp4s0: 181387442  175377    0    0    0     0          0      1159 10438646   44322    0    0    0     0       0          0
wlan0:1000 5 0 0 0 0 0 0 2000 7 0 0 0 0 0 0
tailscale0:      86       1    0    0    0     0          0         0      624      11    0    0    0     0       0          0
";

    #[test]
    fn reads_the_network() {
        let cards = |name: &str| matches!(name, "enp4s0" | "wlan0");
        assert_eq!(
            net_bytes(NET_DEV, cards),
            (181_387_442 + 1000, 10_438_646 + 2000)
        );
        // The header lines aren't interfaces.
        assert_eq!(net_bytes(NET_DEV, |name| name.contains('|')), (0, 0));
    }

    #[test]
    fn reads_a_process_io() {
        let io = "rchar: 8971\nwchar: 8\nsyscr: 12\nsyscw: 1\nread_bytes: 4096\nwrite_bytes: 8192\ncancelled_write_bytes: 0\n";
        assert_eq!(process_io(io), Some(12288));
        assert_eq!(process_io("rchar: 1\n"), None);
    }

    #[test]
    fn signals_only_single_processes() {
        assert!(signal(0, 0).is_err());
        assert!(signal(-1, 0).is_err());
        assert!(signal(i64::from(std::process::id()), 0).is_err());
        assert!(signal(i64::from(i32::MAX), 0).is_err());
        // Signal 0 only checks; the test's parent is the user's.
        let parent = std::os::unix::process::parent_id();
        assert_eq!(signal(i64::from(parent), 0), Ok(()));
    }
}

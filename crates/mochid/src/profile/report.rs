//! A recording as one HTML file: timelines of CPU, wake-ups, memory and
//! frames, the busiest threads and modules, and mochid's flame graph, in
//! `$XDG_STATE_HOME/mochi/profiles`. One file, so a tester can send it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::json;

use super::{FramePoint, Group, Point};

const TEMPLATE: &str = include_str!("report.html");

/// What the report says about the machine and the session.
#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub version: String,
    pub build: &'static str,
    pub compositor: String,
    pub outputs: Vec<String>,
    pub modules: Vec<String>,
    pub kernel: String,
    pub cpu: String,
    pub cores: usize,
}

impl Meta {
    /// The machine's part, read from `/proc`.
    pub fn machine() -> (String, String, usize) {
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|kernel| kernel.trim().to_owned())
            .unwrap_or_default();
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let cpu = cpuinfo
            .lines()
            .find_map(|line| line.strip_prefix("model name"))
            .and_then(|rest| rest.split_once(':'))
            .map(|(_, name)| name.trim().to_owned())
            .unwrap_or_default();
        let cores = std::thread::available_parallelism().map_or(1, usize::from);
        (kernel, cpu, cores)
    }
}

#[derive(Debug)]
pub struct Recorded<'a> {
    pub meta: &'a Meta,
    /// When it started, in the profiler's seconds.
    pub start: f64,
    pub seconds: f64,
    pub points: &'a [Point],
    pub frames: &'a [FramePoint],
    /// The flame graph's SVG, or why there's none.
    pub flame: Result<&'a [u8], &'a str>,
}

/// Writes the report, and returns its path.
pub fn write(recorded: &Recorded) -> Result<PathBuf, String> {
    let dir = mochi_core::config::state_dir()
        .ok_or("can't find the state directory: set XDG_STATE_HOME or HOME")?
        .join("profiles");
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("can't create {}: {error}", dir.display()))?;
    let path = dir.join(format!("profile-{}.html", stamp()));
    std::fs::write(&path, html(recorded))
        .map_err(|error| format!("can't write {}: {error}", path.display()))?;
    Ok(path)
}

/// The report's page: the template with the data in it.
fn html(recorded: &Recorded) -> String {
    let count = recorded.points.len().max(1) as f64;
    // Each thread's and each module's average over the recording.
    let mut threads: BTreeMap<(Group, &str, &str), f64> = BTreeMap::new();
    let mut modules: BTreeMap<&str, f64> = BTreeMap::new();
    for point in recorded.points {
        for thread in &point.threads {
            *threads
                .entry((thread.group, &thread.process, &thread.thread))
                .or_default() += thread.cpu / count;
        }
        for (module, cpu) in &point.modules {
            *modules.entry(module).or_default() += cpu / count;
        }
    }
    let mut threads: Vec<_> = threads
        .into_iter()
        .map(|((group, process, thread), cpu)| {
            json!({ "group": group, "process": process, "thread": thread, "cpu": cpu })
        })
        .collect();
    let cpu = |value: &serde_json::Value| value["cpu"].as_f64().unwrap_or(0.0);
    threads.sort_by(|a, b| cpu(b).total_cmp(&cpu(a)));
    let mut modules: Vec<_> = modules
        .into_iter()
        .map(|(module, cpu)| json!({ "module": module, "cpu": cpu }))
        .collect();
    modules.sort_by(|a, b| cpu(b).total_cmp(&cpu(a)));

    let points: Vec<_> = recorded
        .points
        .iter()
        .map(|point| {
            json!({
                "t": point.t - recorded.start,
                "cpu": point.cpu,
                "wakeups": point.wakeups,
                "memory": point.memory,
            })
        })
        .collect();
    let frames: Vec<_> = recorded
        .frames
        .iter()
        .map(|frame| {
            json!({
                "t": frame.t - recorded.start,
                "output": frame.output,
                "surface": frame.surface,
                "fps": frame.fps,
            })
        })
        .collect();
    let data = json!({
        "meta": recorded.meta,
        "seconds": recorded.seconds,
        "interval": super::INTERVAL.as_secs_f64(),
        "points": points,
        "frames": frames,
        "threads": threads,
        "modules": modules,
        "flame": recorded.flame.ok().map(base64),
        "flame_error": recorded.flame.err(),
    });
    // `</` would end the script the data sits in.
    let data = data.to_string().replace("</", "<\\/");
    TEMPLATE.replace("/*DATA*/null", &data)
}

/// The local time, as `2026-10-12-221530`.
fn stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let time = libc::time_t::try_from(now).unwrap_or_default();
    // SAFETY: `tm` is plain data that `localtime_r` fills in completely; an
    // all-zero value is a valid one to start from.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are valid for the call, and `localtime_r` is the
    // thread-safe form.
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return now.to_string();
    }
    format!(
        "{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = u32::from(chunk[0]) << 16
            | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> shift & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_page_holds_the_data_and_the_flame_graph() {
        let meta = Meta {
            version: "0.3.0".into(),
            build: "release",
            compositor: "Hyprland".into(),
            outputs: vec!["DP-3".into()],
            modules: vec!["media".into()],
            kernel: "7.2".into(),
            cpu: "Some CPU".into(),
            cores: 12,
        };
        let page = html(&Recorded {
            meta: &meta,
            start: 10.0,
            seconds: 5.0,
            points: &[],
            frames: &[FramePoint {
                t: 11.0,
                output: "DP-3".into(),
                surface: "island".into(),
                fps: 30,
            }],
            flame: Ok(b"<svg></svg>"),
        });
        assert!(!page.contains("/*DATA*/null"));
        assert!(page.contains(r#""fps":30"#));
        assert!(page.contains(r#""t":1.0"#));
        assert!(page.contains(&base64(b"<svg></svg>")));
        // Nothing in the data can close its script.
        let data = page.split("const D = ").nth(1).unwrap();
        assert!(!data.split("</script>").next().unwrap().contains("</svg"));
    }
}

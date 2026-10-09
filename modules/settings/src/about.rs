//! The About page: which Mochi runs, on what, and how it's doing, for a bug
//! report. It's read when the page shows, from the daemon's status, /proc
//! and `quickshell --version`, and Copy gives it as text.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub const REPOSITORY: &str = "https://github.com/DavidutzDev/mochi";
pub const DOCUMENTATION: &str = "https://davidutzdev.github.io/mochi/";
pub const ISSUES: &str = "https://github.com/DavidutzDev/mochi/issues/new";

/// The links the page may open; nothing else goes to `xdg-open`.
pub fn link(name: &str) -> Option<&'static str> {
    match name {
        "repository" => Some(REPOSITORY),
        "documentation" => Some(DOCUMENTATION),
        "issue" => Some(ISSUES),
        _ => None,
    }
}

/// Everything the page shows.
pub async fn gather(socket: Option<PathBuf>, config: Option<PathBuf>) -> Value {
    let pid = std::process::id();
    let status = match &socket {
        Some(socket) => status(socket).await.map_err(|error| error.to_string()),
        None => Err("no socket".to_owned()),
    };
    let (status, status_error) = match status {
        Ok(status) => (status, Value::Null),
        Err(error) => (Value::Null, Value::String(error)),
    };
    json!({
        "mochi": {
            "version": mochi_core::version::VERSION,
            "build": if cfg!(debug_assertions) { "debug" } else { "release" },
            "revision": revision(),
            "api": mochi_protocol::API,
            "pid": pid,
            "uptime": uptime(pid),
            "memory": memory(pid),
            "quickshell": quickshell().await,
            "quickshell_wanted": mochi_core::supervisor::QUICKSHELL_VERSION,
            "config": config.map(|path| path.display().to_string()),
            "socket": socket.map(|path| path.display().to_string()),
        },
        "system": {
            "os": os(),
            "kernel": read_trimmed("/proc/sys/kernel/osrelease"),
            "arch": std::env::consts::ARCH,
            "cpu": cpu(),
            "cores": std::thread::available_parallelism().map(|n| n.get()).ok(),
            "memory": meminfo("MemTotal"),
        },
        "session": {
            "desktop": std::env::var("XDG_CURRENT_DESKTOP").ok(),
            "type": std::env::var("XDG_SESSION_TYPE").ok(),
            "display": std::env::var("WAYLAND_DISPLAY").ok(),
        },
        "status": status,
        "status_error": status_error,
        "processes": children(pid),
    })
}

/// The daemon's `status`, asked over its socket like `mochi status`.
async fn status(socket: &Path) -> anyhow_lite::Result<Value> {
    use mochi_protocol::{API, ClientMessage, Role};

    let stream = tokio::net::UnixStream::connect(socket).await?;
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    for message in [
        ClientMessage::Hello {
            api: API,
            role: Role::Ctl,
        },
        ClientMessage::Status,
    ] {
        let mut line = serde_json::to_string(&message)?;
        line.push('\n');
        writer.write_all(line.as_bytes()).await?;
        let answer = tokio::time::timeout(Duration::from_secs(3), lines.next_line())
            .await
            .map_err(|_| "the daemon didn't answer")??
            .ok_or("the daemon closed the connection")?;
        let answer: Value = serde_json::from_str(&answer)?;
        match answer["type"].as_str() {
            Some("status") => return Ok(answer["status"].clone()),
            Some("error") => {
                return Err(answer["message"].as_str().unwrap_or("an error").into());
            }
            _ => {}
        }
    }
    Err("no status in the answer".into())
}

/// `quickshell --version`'s first line.
async fn quickshell() -> Option<String> {
    let run = tokio::process::Command::new("quickshell")
        .arg("--version")
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(3), run)
        .await
        .ok()?
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
}

/// The revision of the source tree, for a debug build run from it.
fn revision() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .args(["describe", "--always", "--dirty=*", "--abbrev=7"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// The distribution's name, from os-release.
fn os() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .ok()?;
    let field = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .map(|value| value.trim_matches('"').to_owned())
    };
    field("PRETTY_NAME").or_else(|| field("NAME"))
}

fn cpu() -> Option<String> {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    text.lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            matches!(key.trim(), "model name" | "Model" | "Hardware").then(|| value.trim())
        })
        .map(|name| name.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// A `/proc/meminfo` line, in bytes.
fn meminfo(key: &str) -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    kilobytes(&text, key)
}

/// A `key: <n> kB` line from a /proc file, in bytes.
fn kilobytes(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix(key)?.strip_prefix(':')?;
        let number: u64 = rest.trim().trim_end_matches("kB").trim().parse().ok()?;
        Some(number * 1024)
    })
}

/// A process's resident memory, in bytes.
fn memory(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    kilobytes(&text, "VmRSS")
}

/// The fields of `/proc/<pid>/stat` after the command's name, which may
/// hold spaces.
fn stat(pid: u32) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, rest) = text.rsplit_once(')')?;
    Some(rest.split_whitespace().map(str::to_owned).collect())
}

/// How long a process has run, in seconds.
fn uptime(pid: u32) -> Option<u64> {
    // Field 22, the start in clock ticks after boot; the name was field 2.
    let start: u64 = stat(pid)?.get(19)?.parse().ok()?;
    // SAFETY: sysconf has no memory-safety preconditions.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    let ticks = u64::try_from(ticks).ok().filter(|ticks| *ticks > 0)?;
    let boot: f64 = read_trimmed("/proc/uptime")?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some((boot as u64).saturating_sub(start / ticks))
}

/// The processes the daemon started and that still run: Quickshell and
/// plugin backends, with their memory.
fn children(parent: u32) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut out: Vec<Value> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            stat(*pid)
                .and_then(|fields| fields.get(1)?.parse::<u32>().ok())
                .is_some_and(|ppid| ppid == parent)
        })
        .map(|pid| {
            let name = std::fs::read(format!("/proc/{pid}/cmdline"))
                .ok()
                .and_then(|line| {
                    let first = line.split(|byte| *byte == 0).next()?;
                    let first = String::from_utf8_lossy(first).into_owned();
                    let name = Path::new(&first).file_name()?.to_str()?.to_owned();
                    // Nix's wrappers are named .<name>-wrapped.
                    Some(
                        name.trim_start_matches('.')
                            .trim_end_matches("-wrapped")
                            .to_owned(),
                    )
                })
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| pid.to_string());
            json!({ "pid": pid, "name": name, "memory": memory(pid) })
        })
        .collect();
    out.sort_by_key(|process| process["pid"].as_u64());
    out
}

/// The page as text, for a bug report.
pub fn text(about: &Value) -> String {
    let get = |section: &str, key: &str| -> String {
        match &about[section][key] {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            _ => "unknown".to_owned(),
        }
    };
    let mib = |value: &Value| {
        value.as_u64().map_or("unknown".to_owned(), |bytes| {
            format!("{} MiB", bytes / (1024 * 1024))
        })
    };
    let mut out = format!(
        "Mochi {} ({} build{})\n",
        get("mochi", "version"),
        get("mochi", "build"),
        about["mochi"]["revision"]
            .as_str()
            .map(|revision| format!(", {revision}"))
            .unwrap_or_default()
    );
    out.push_str(&format!("Quickshell: {}\n", get("mochi", "quickshell")));
    out.push_str(&format!(
        "System: {}, Linux {}, {}\n",
        get("system", "os"),
        get("system", "kernel"),
        get("system", "arch")
    ));
    out.push_str(&format!(
        "CPU: {} ({} cores), memory {}\n",
        get("system", "cpu"),
        get("system", "cores"),
        mib(&about["system"]["memory"])
    ));
    let status = &about["status"];
    out.push_str(&format!(
        "Session: {} ({}), compositor {}\n",
        get("session", "desktop"),
        get("session", "type"),
        status["compositor"]["backend"]
            .as_str()
            .unwrap_or("unknown")
    ));
    if let Some(outputs) = status["compositor"]["outputs"].as_array() {
        let names: Vec<&str> = outputs
            .iter()
            .filter_map(|output| output["name"].as_str().or(output.as_str()))
            .collect();
        out.push_str(&format!("Screens: {}\n", names.join(", ")));
    }
    if let Some(modules) = status["modules"].as_array() {
        let ids: Vec<&str> = modules.iter().filter_map(Value::as_str).collect();
        out.push_str(&format!("Modules: {}\n", ids.join(", ")));
    }
    if let Some(plugins) = status["plugins"].as_array().filter(|list| !list.is_empty()) {
        let list: Vec<String> = plugins
            .iter()
            .map(|plugin| {
                let mut line = format!(
                    "{} {}",
                    plugin["id"].as_str().unwrap_or("?"),
                    plugin["state"].as_str().unwrap_or("?")
                );
                if let Some(message) = plugin["message"].as_str() {
                    line.push_str(&format!(" ({message})"));
                }
                line
            })
            .collect();
        out.push_str(&format!("Plugins: {}\n", list.join(", ")));
    }
    out.push_str(&format!(
        "mochid: pid {}, up {} s, {}\n",
        get("mochi", "pid"),
        get("mochi", "uptime"),
        mib(&about["mochi"]["memory"])
    ));
    for process in about["processes"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "  {} (pid {}): {}\n",
            process["name"].as_str().unwrap_or("?"),
            process["pid"],
            mib(&process["memory"])
        ));
    }
    out
}

/// Errors as text, without pulling anyhow into a module.
mod anyhow_lite {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_kilobyte_lines() {
        let text = "Name:\tmochid\nVmRSS:\t   51200 kB\n";
        assert_eq!(kilobytes(text, "VmRSS"), Some(51200 * 1024));
        assert_eq!(kilobytes(text, "VmSwap"), None);
    }

    #[test]
    fn only_known_links_open() {
        assert_eq!(link("repository"), Some(REPOSITORY));
        assert_eq!(link("https://example.com"), None);
    }

    #[test]
    fn the_text_names_what_a_bug_report_needs() {
        let about = json!({
            "mochi": { "version": "0.0.8", "build": "release", "quickshell": "Quickshell 0.3.1", "pid": 7, "uptime": 60, "memory": 52428800 },
            "system": { "os": "NixOS 26.05", "kernel": "7.2.9", "arch": "x86_64", "cpu": "Ryzen", "cores": 16, "memory": 34359738368u64 },
            "session": { "desktop": "Hyprland", "type": "wayland" },
            "status": { "compositor": { "backend": "hyprland", "outputs": [{ "name": "DP-3" }] }, "modules": ["idle"], "plugins": [] },
            "processes": [{ "pid": 8, "name": "quickshell", "memory": 104857600 }],
        });
        let text = text(&about);
        assert!(text.starts_with("Mochi 0.0.8 (release build)\n"), "{text}");
        assert!(
            text.contains("System: NixOS 26.05, Linux 7.2.9, x86_64"),
            "{text}"
        );
        assert!(text.contains("compositor hyprland"), "{text}");
        assert!(text.contains("Screens: DP-3"), "{text}");
        assert!(text.contains("  quickshell (pid 8): 100 MiB"), "{text}");
    }
}

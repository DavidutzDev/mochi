//! Reads the process tree under mochid from `/proc`: each process's memory
//! and context switches, and each thread's CPU time, in nanoseconds, from
//! its `schedstat`. A thread of its own does it, only while profiling.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

/// One reading of the whole tree.
#[derive(Debug, Clone)]
pub struct Sample {
    pub at: Instant,
    pub processes: Vec<Process>,
    pub threads: Vec<Thread>,
    /// Each module's time in its own task so far, in nanoseconds.
    pub modules: Vec<(&'static str, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    pub comm: String,
    pub rss_kb: u64,
    /// Voluntary and involuntary context switches so far: how often it
    /// woke up, more or less.
    pub switches: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub pid: u32,
    pub tid: u32,
    pub name: String,
    pub cpu_ns: u64,
}

/// Reads `root` and every process under it.
pub fn read(root: u32) -> Sample {
    let at = Instant::now();
    let mut processes = Vec::new();
    let mut threads = Vec::new();
    for (pid, comm) in tree(root) {
        let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
            continue;
        };
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0)
        };
        processes.push(Process {
            pid,
            comm,
            rss_kb: field("VmRSS:"),
            switches: field("voluntary_ctxt_switches:") + field("nonvoluntary_ctxt_switches:"),
        });
        let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
            continue;
        };
        for task in tasks.flatten() {
            let Some(tid) = task.file_name().to_str().and_then(|name| name.parse().ok()) else {
                continue;
            };
            let read = |file: &str| std::fs::read_to_string(task.path().join(file)).ok();
            let Some(cpu_ns) =
                read("schedstat").and_then(|line| line.split_whitespace().next()?.parse().ok())
            else {
                continue;
            };
            let name = read("comm")
                .map(|name| name.trim().to_owned())
                .unwrap_or_default();
            threads.push(Thread {
                pid,
                tid,
                name,
                cpu_ns,
            });
        }
    }
    Sample {
        at,
        processes,
        threads,
        modules: super::module_times(),
    }
}

/// `root` and its descendants, by pid, with their names. Each thread's
/// `children` file lists the processes it started, so only the tree is
/// read; a kernel without them gets a pass over every process.
fn tree(root: u32) -> Vec<(u32, String)> {
    if !std::path::Path::new(&format!("/proc/{root}/task/{root}/children")).exists() {
        return tree_by_parents(root);
    }
    let mut found = Vec::new();
    let mut queue = vec![root];
    while let Some(pid) = queue.pop() {
        let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) else {
            continue;
        };
        found.push((pid, comm.trim().to_owned()));
        let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
            continue;
        };
        for task in tasks.flatten() {
            let children =
                std::fs::read_to_string(task.path().join("children")).unwrap_or_default();
            queue.extend(
                children
                    .split_whitespace()
                    .filter_map(|child| child.parse::<u32>().ok()),
            );
        }
    }
    found.sort_unstable();
    found.dedup_by_key(|(pid, _)| *pid);
    found
}

/// The same, from every process's parent.
fn tree_by_parents(root: u32) -> Vec<(u32, String)> {
    let mut parents = BTreeMap::new();
    let mut names = BTreeMap::new();
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some((comm, ppid)) = parse_stat(&stat) {
            parents.insert(pid, ppid);
            names.insert(pid, comm);
        }
    }
    let mut inside = BTreeSet::from([root]);
    loop {
        let before = inside.len();
        for (pid, parent) in &parents {
            if inside.contains(parent) {
                inside.insert(*pid);
            }
        }
        if inside.len() == before {
            break;
        }
    }
    inside
        .into_iter()
        .filter_map(|pid| Some((pid, names.remove(&pid)?)))
        .collect()
}

/// The name and the parent from `/proc/<pid>/stat`. The name is between
/// the first `(` and the last `)`, as it may hold either.
fn parse_stat(stat: &str) -> Option<(String, u32)> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let comm = stat.get(open + 1..close)?.to_owned();
    let mut rest = stat.get(close + 1..)?.split_whitespace();
    let _state = rest.next()?;
    let ppid = rest.next()?.parse().ok()?;
    Some((comm, ppid))
}

/// Reads the tree every `interval` on a thread of its own and sends each
/// reading, until `stop` is set or the receiver goes away.
pub fn spawn(
    root: u32,
    interval: Duration,
    sender: UnboundedSender<Sample>,
    stop: Arc<AtomicBool>,
) {
    let spawned = std::thread::Builder::new()
        .name("mochi-profile".into())
        .spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if sender.send(read(root)).is_err() {
                    break;
                }
                std::thread::sleep(interval);
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "can't start the profiler");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_names_with_spaces_and_parens() {
        assert_eq!(
            parse_stat("4321 (Web (Content)) S 1200 4321 4321 0 -1"),
            Some(("Web (Content)".into(), 1200))
        );
        assert_eq!(parse_stat("garbage"), None);
    }

    #[test]
    fn reads_this_process_and_its_child() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        let sample = read(std::process::id());
        child.kill().unwrap();
        child.wait().unwrap();

        let pids: Vec<u32> = sample.processes.iter().map(|process| process.pid).collect();
        assert!(pids.contains(&std::process::id()));
        assert!(pids.contains(&child.id()));
        let me = sample
            .processes
            .iter()
            .find(|process| process.pid == std::process::id())
            .unwrap();
        assert!(me.rss_kb > 0);
        // The test's own thread has run.
        assert!(
            sample
                .threads
                .iter()
                .any(|thread| thread.pid == std::process::id() && thread.cpu_ns > 0)
        );
    }
}

//! The stopwatch without the daemon: the time it ran, whether it runs, and
//! the laps, and the runs it made before, each kept when it's reset. Every
//! call takes the time as milliseconds since the epoch, so the tests don't
//! wait.
//!
//! The time is the wall clock's, so the stopwatch counts through a suspend,
//! and the views count up from when it started without asking the daemon.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The most laps it keeps.
pub const MOST_LAPS: usize = 99;
/// The most past runs the `history` setting may keep.
pub const MOST_RUNS: u32 = 50;

/// How finely the time shows: to the tenth of a second, or the hundredth.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    /// To the tenth of a second, like 1:02.3.
    #[default]
    Tenths,
    /// To the hundredth, like 1:02.34.
    Hundredths,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stopwatch {
    /// When it last started, while it runs.
    pub since_ms: Option<u64>,
    /// The time it ran before `since_ms`.
    pub banked_ms: u64,
    /// The time it showed at each lap, in order.
    pub laps: Vec<u64>,
}

impl Stopwatch {
    pub fn running(&self) -> bool {
        self.since_ms.is_some()
    }

    /// The time it shows at `now`.
    pub fn elapsed(&self, now: u64) -> u64 {
        self.banked_ms + self.since_ms.map_or(0, |since| now.saturating_sub(since))
    }

    /// Starts, or carries on from where it paused. Starting a running
    /// stopwatch changes nothing.
    pub fn start(&mut self, now: u64) {
        if self.since_ms.is_none() {
            self.since_ms = Some(now);
        }
    }

    pub fn pause(&mut self, now: u64) {
        if self.since_ms.is_some() {
            self.banked_ms = self.elapsed(now);
            self.since_ms = None;
        }
    }

    pub fn toggle(&mut self, now: u64) {
        if self.running() {
            self.pause(now);
        } else {
            self.start(now);
        }
    }

    /// Notes the time it shows, while it runs.
    pub fn lap(&mut self, now: u64) -> Result<(), String> {
        if !self.running() {
            return Err("the stopwatch isn't running".to_owned());
        }
        if self.laps.len() >= MOST_LAPS {
            return Err(format!("the stopwatch keeps up to {MOST_LAPS} laps"));
        }
        self.laps.push(self.elapsed(now));
        Ok(())
    }

    /// Back to zero, without laps. Returns the run it made, if it ran,
    /// ended at `ended`, in seconds since the epoch.
    pub fn reset(&mut self, now: u64, ended: i64) -> Option<Run> {
        let total_ms = self.elapsed(now);
        let laps = std::mem::take(&mut self.laps);
        *self = Self::default();
        (total_ms > 0).then_some(Run {
            ended,
            total_ms,
            laps,
        })
    }

    /// What `mochi ipc clock stopwatch` prints: the time, and the laps.
    pub fn status(&self, now: u64, precision: Precision) -> String {
        let state = if self.running() {
            "running"
        } else if self.elapsed(now) > 0 {
            "paused"
        } else {
            "stopped"
        };
        let mut lines = vec![format!("{} {state}", clock(self.elapsed(now), precision))];
        for (index, (split, total)) in splits(&self.laps).iter().zip(&self.laps).enumerate() {
            lines.push(format!(
                "lap {} {} {}",
                index + 1,
                clock(*split, precision),
                clock(*total, precision)
            ));
        }
        lines.join("\n")
    }
}

/// A run the stopwatch made, kept when it was reset.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Run {
    /// When it was reset, in seconds since the epoch.
    pub ended: i64,
    /// The time it showed.
    pub total_ms: u64,
    /// The time it showed at each lap, in order.
    pub laps: Vec<u64>,
}

/// The past runs, newest first, kept in
/// `$XDG_STATE_HOME/mochi/stopwatch-runs.json`: unlike the stopwatch, they
/// outlast a logout.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Runs(pub Vec<Run>);

impl Runs {
    pub fn path() -> Option<PathBuf> {
        Some(mochi_core::config::state_dir()?.join("stopwatch-runs.json"))
    }

    /// The runs in `path`, or none when it's missing or doesn't read.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&text)
            .map(Self)
            .inspect_err(|error| tracing::warn!(%error, "ignoring the saved stopwatch runs"))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temporary = path.with_extension("json.new");
        std::fs::write(&temporary, serde_json::to_vec(&self.0).unwrap_or_default())?;
        std::fs::rename(temporary, path)
    }

    /// Keeps `run` first, and the newest `most` in all.
    pub fn add(&mut self, run: Run, most: usize) {
        self.0.insert(0, run);
        self.0.truncate(most);
    }

    /// The run numbered `number`, 1 for the newest.
    pub fn get(&self, number: i64) -> Result<&Run, String> {
        Self::index(number)
            .and_then(|index| self.0.get(index))
            .ok_or_else(|| self.missing(number))
    }

    /// Forgets the run numbered `number`, or all of them.
    pub fn forget(&mut self, number: Option<i64>) -> Result<(), String> {
        match number {
            None => self.0.clear(),
            Some(number) => match Self::index(number).filter(|index| *index < self.0.len()) {
                Some(index) => {
                    self.0.remove(index);
                }
                None => return Err(self.missing(number)),
            },
        }
        Ok(())
    }

    fn index(number: i64) -> Option<usize> {
        usize::try_from(number).ok()?.checked_sub(1)
    }

    fn missing(&self, number: i64) -> String {
        match self.0.len() {
            0 => "the stopwatch has no past runs".to_owned(),
            1 => format!("there's no run {number}, only 1"),
            count => format!("there's no run {number}; they go from 1 to {count}"),
        }
    }

    /// What `mochi ipc clock runs` prints: one line each, with its number,
    /// and `when` writing when it ended.
    pub fn listing(&self, precision: Precision, when: impl Fn(i64) -> String) -> String {
        if self.0.is_empty() {
            return "no past runs".to_owned();
        }
        let lines: Vec<String> = self
            .0
            .iter()
            .enumerate()
            .map(|(index, run)| {
                let laps = match run.laps.len() {
                    0 => String::new(),
                    1 => ", 1 lap".to_owned(),
                    count => format!(", {count} laps"),
                };
                format!(
                    "{}  {}  {}{laps}",
                    index + 1,
                    when(run.ended),
                    clock(run.total_ms, precision)
                )
            })
            .collect();
        lines.join("\n")
    }
}

/// How long each lap took, from the time at each, in order.
pub fn splits(laps: &[u64]) -> Vec<u64> {
    let mut before = 0;
    laps.iter()
        .map(|total| {
            let split = total.saturating_sub(before);
            before = *total;
            split
        })
        .collect()
}

/// A run as it's copied: `title` with the time, then a line a lap with how
/// long it took and the time at its end.
pub fn report(title: &str, total: u64, laps: &[u64], precision: Precision) -> String {
    let mut lines = vec![format!("{title}: {}", clock(total, precision))];
    for (index, (split, at)) in splits(laps).iter().zip(laps).enumerate() {
        lines.push(format!(
            "Lap {}  {}  {}",
            index + 1,
            clock(*split, precision),
            clock(*at, precision)
        ));
    }
    lines.join("\n")
}

/// Milliseconds as m:ss.t, or h:mm:ss.t from an hour; with a second digit
/// for hundredths.
pub fn clock(ms: u64, precision: Precision) -> String {
    let fraction = match precision {
        Precision::Tenths => format!("{}", ms / 100 % 10),
        Precision::Hundredths => format!("{:02}", ms / 10 % 100),
    };
    let seconds = ms / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{fraction}")
    } else {
        format!("{minutes}:{seconds:02}.{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_000_000_000;

    #[test]
    fn counts_while_it_runs() {
        let mut watch = Stopwatch::default();
        assert_eq!(watch.elapsed(NOW), 0);
        watch.start(NOW);
        assert_eq!(watch.elapsed(NOW + 1500), 1500);
        // Starting again changes nothing.
        watch.start(NOW + 1000);
        assert_eq!(watch.elapsed(NOW + 1500), 1500);
        watch.pause(NOW + 2000);
        assert_eq!(watch.elapsed(NOW + 9000), 2000);
        watch.toggle(NOW + 10_000);
        assert_eq!(watch.elapsed(NOW + 10_500), 2500);
        let run = watch.reset(NOW + 11_000, 1_790_000_011).unwrap();
        assert_eq!(run.total_ms, 3000);
        assert_eq!(watch, Stopwatch::default());
        // A stopwatch that didn't run makes no run.
        assert_eq!(watch.reset(NOW + 12_000, 1_790_000_012), None);
    }

    #[test]
    fn laps_note_the_time_and_how_long_each_took() {
        let mut watch = Stopwatch::default();
        assert!(watch.lap(NOW).is_err());
        watch.start(NOW);
        watch.lap(NOW + 61_000).unwrap();
        watch.lap(NOW + 91_500).unwrap();
        // A pause doesn't count toward the lap.
        watch.pause(NOW + 100_000);
        assert!(watch.lap(NOW + 120_000).is_err());
        watch.start(NOW + 200_000);
        watch.lap(NOW + 210_000).unwrap();
        assert_eq!(watch.laps, [61_000, 91_500, 110_000]);
        assert_eq!(splits(&watch.laps), [61_000, 30_500, 18_500]);
        assert_eq!(
            watch.status(NOW + 210_000, Precision::Hundredths),
            "1:50.00 running\nlap 1 1:01.00 1:01.00\nlap 2 0:30.50 1:31.50\nlap 3 0:18.50 1:50.00"
        );
        assert_eq!(
            watch.status(NOW + 210_000, Precision::Tenths),
            "1:50.0 running\nlap 1 1:01.0 1:01.0\nlap 2 0:30.5 1:31.5\nlap 3 0:18.5 1:50.0"
        );
        // Reset keeps the run, with its laps.
        let run = watch.reset(NOW + 215_000, 1_790_000_215).unwrap();
        assert_eq!(run.total_ms, 115_000);
        assert_eq!(run.laps, [61_000, 91_500, 110_000]);
        assert_eq!(
            report("Stopwatch", run.total_ms, &run.laps, Precision::Tenths),
            "Stopwatch: 1:55.0\nLap 1  1:01.0  1:01.0\nLap 2  0:30.5  1:31.5\nLap 3  0:18.5  1:50.0"
        );
    }

    #[test]
    fn keeps_a_bounded_number_of_laps() {
        let mut watch = Stopwatch::default();
        watch.start(NOW);
        for lap in 0..MOST_LAPS as u64 {
            watch.lap(NOW + lap).unwrap();
        }
        assert!(watch.lap(NOW + 1000).is_err());
    }

    #[test]
    fn reads_like_a_stopwatch() {
        assert_eq!(clock(0, Precision::Hundredths), "0:00.00");
        assert_eq!(clock(61_234, Precision::Hundredths), "1:01.23");
        assert_eq!(clock(3_723_450, Precision::Hundredths), "1:02:03.45");
        assert_eq!(clock(61_234, Precision::Tenths), "1:01.2");
        assert_eq!(clock(3_723_450, Precision::Tenths), "1:02:03.4");
        let watch = Stopwatch {
            banked_ms: 5000,
            ..Stopwatch::default()
        };
        assert_eq!(watch.status(NOW, Precision::Tenths), "0:05.0 paused");
        assert_eq!(
            Stopwatch::default().status(NOW, Precision::Hundredths),
            "0:00.00 stopped"
        );
    }

    #[test]
    fn survives_a_save() {
        let mut watch = Stopwatch::default();
        watch.start(NOW);
        watch.lap(NOW + 1000).unwrap();
        let text = serde_json::to_string(&watch).unwrap();
        let back: Stopwatch = serde_json::from_str(&text).unwrap();
        assert_eq!(back, watch);
        // Read after a restart, it ran meanwhile.
        assert_eq!(back.elapsed(NOW + 60_000), 60_000);
    }

    #[test]
    fn keeps_the_newest_runs() {
        let run = |ended| Run {
            ended,
            total_ms: 1000,
            laps: Vec::new(),
        };
        let mut runs = Runs::default();
        assert!(runs.get(1).is_err());
        for ended in 1..=4 {
            runs.add(run(ended), 3);
        }
        let ended: Vec<i64> = runs.0.iter().map(|run| run.ended).collect();
        assert_eq!(ended, [4, 3, 2]);
        assert_eq!(runs.get(1).unwrap().ended, 4);
        assert!(runs.get(0).is_err() && runs.get(4).is_err() && runs.get(-1).is_err());
        runs.forget(Some(2)).unwrap();
        assert_eq!(runs.0.len(), 2);
        assert_eq!(runs.get(2).unwrap().ended, 2);
        assert!(runs.forget(Some(3)).is_err());
        assert_eq!(
            runs.listing(Precision::Tenths, |at| format!("t{at}")),
            "1  t4  0:01.0\n2  t2  0:01.0"
        );
        runs.forget(None).unwrap();
        assert_eq!(
            runs.listing(Precision::Tenths, |_| String::new()),
            "no past runs"
        );
    }

    #[test]
    fn runs_survive_a_save() {
        let dir = std::env::temp_dir().join(format!("mochi-runs-{}", std::process::id()));
        let path = dir.join("stopwatch-runs.json");
        let mut runs = Runs::default();
        runs.add(
            Run {
                ended: 5,
                total_ms: 1234,
                laps: vec![500],
            },
            10,
        );
        runs.save(&path).unwrap();
        assert_eq!(Runs::load(&path), runs);
        std::fs::remove_dir_all(dir).unwrap();
        assert_eq!(Runs::load(&path), Runs::default());
    }
}

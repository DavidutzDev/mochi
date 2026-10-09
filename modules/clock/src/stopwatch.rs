//! The stopwatch without the daemon: the time it ran, whether it runs, and
//! the laps. Every call takes the time as milliseconds since the epoch, so
//! the tests don't wait.
//!
//! The time is the wall clock's, so the stopwatch counts through a suspend,
//! and the views count up from when it started without asking the daemon.

use serde::{Deserialize, Serialize};

/// The most laps it keeps.
pub const MOST_LAPS: usize = 99;

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

    /// Back to zero, without laps.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// How long each lap took, in order.
    pub fn splits(&self) -> Vec<u64> {
        let mut before = 0;
        self.laps
            .iter()
            .map(|total| {
                let split = total - before;
                before = *total;
                split
            })
            .collect()
    }

    /// What `mochi ipc clock stopwatch` prints: the time, and the laps.
    pub fn status(&self, now: u64) -> String {
        let state = if self.running() {
            "running"
        } else if self.elapsed(now) > 0 {
            "paused"
        } else {
            "stopped"
        };
        let mut lines = vec![format!("{} {state}", clock(self.elapsed(now)))];
        for (index, (split, total)) in self.splits().iter().zip(&self.laps).enumerate() {
            lines.push(format!(
                "lap {} {} {}",
                index + 1,
                clock(*split),
                clock(*total)
            ));
        }
        lines.join("\n")
    }
}

/// Milliseconds as m:ss.cc, or h:mm:ss.cc from an hour.
pub fn clock(ms: u64) -> String {
    let hundredths = ms / 10 % 100;
    let seconds = ms / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{hundredths:02}")
    } else {
        format!("{minutes}:{seconds:02}.{hundredths:02}")
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
        watch.reset();
        assert_eq!(watch, Stopwatch::default());
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
        assert_eq!(watch.splits(), [61_000, 30_500, 18_500]);
        assert_eq!(
            watch.status(NOW + 210_000),
            "1:50.00 running\nlap 1 1:01.00 1:01.00\nlap 2 0:30.50 1:31.50\nlap 3 0:18.50 1:50.00"
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
        assert_eq!(clock(0), "0:00.00");
        assert_eq!(clock(61_234), "1:01.23");
        assert_eq!(clock(3_723_450), "1:02:03.45");
        let watch = Stopwatch {
            banked_ms: 5000,
            ..Stopwatch::default()
        };
        assert_eq!(watch.status(NOW), "0:05.00 paused");
        assert_eq!(Stopwatch::default().status(NOW), "0:00.00 stopped");
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
}

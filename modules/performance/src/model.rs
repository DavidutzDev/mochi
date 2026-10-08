//! When a reading deserves a notice or the critical bubble: only when it
//! stays high for a while, so a spike says nothing, and once until it
//! comes back down.

use std::time::{Duration, Instant};

/// The levels of one reading, from the settings. 0 turns one off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub notice: u32,
    pub critical: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            notice: 85,
            critical: 95,
        }
    }
}

/// How far a reading must fall under a level before it counts as down
/// again, so one hovering at the level doesn't say it over and over.
const SETTLE: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// It stayed over the notice level.
    High,
    /// It stayed over the critical level: the bubble shows.
    Critical,
    /// It came down from critical: the bubble goes.
    Calm,
}

/// One reading's state, like the CPU's use.
#[derive(Debug, Default)]
pub struct Watch {
    high_since: Option<Instant>,
    told: bool,
    critical_since: Option<Instant>,
    pub critical: bool,
}

impl Watch {
    pub fn update(
        &mut self,
        value: f64,
        limits: Limits,
        sustain: Duration,
        now: Instant,
    ) -> Option<Change> {
        let over = |level: u32| level > 0 && value >= f64::from(level);
        let under = |level: u32| level == 0 || value < f64::from(level) - SETTLE;

        if over(limits.critical) {
            let since = *self.critical_since.get_or_insert(now);
            if !self.critical && now.duration_since(since) >= sustain {
                self.critical = true;
                self.told = true;
                return Some(Change::Critical);
            }
        } else {
            self.critical_since = None;
            if self.critical && under(limits.critical) {
                self.critical = false;
                return Some(Change::Calm);
            }
        }

        if over(limits.notice) {
            let since = *self.high_since.get_or_insert(now);
            if !self.told && now.duration_since(since) >= sustain {
                self.told = true;
                return Some(Change::High);
            }
        } else {
            self.high_since = None;
            if under(limits.notice) {
                self.told = false;
            }
        }
        None
    }
}

/// The last `CAPACITY` readings, for the page's graphs.
#[derive(Debug, Default)]
pub struct History(std::collections::VecDeque<f64>);

pub const CAPACITY: usize = 60;

impl History {
    pub fn push(&mut self, value: f64) {
        if self.0.len() == CAPACITY {
            self.0.pop_front();
        }
        self.0.push_back((value * 10.0).round() / 10.0);
    }

    pub fn values(&self) -> Vec<f64> {
        self.0.iter().copied().collect()
    }
}

/// Two byte counts that only grow, like a disk's reads and writes, as
/// speeds in bytes a second, with their last readings for a graph.
#[derive(Debug, Default)]
pub struct Flow {
    before: Option<((u64, u64), Instant)>,
    pub speeds: (f64, f64),
    pub histories: (History, History),
}

impl Flow {
    pub fn update(&mut self, totals: (u64, u64), now: Instant) {
        if let Some((before, then)) = self.before {
            let seconds = now.duration_since(then).as_secs_f64();
            if seconds > 0.0 {
                // A count that went back, from a disk or card going away,
                // reads as nothing rather than a huge number.
                let speed = |after: u64, before: u64| after.saturating_sub(before) as f64 / seconds;
                self.speeds = (speed(totals.0, before.0), speed(totals.1, before.1));
                self.histories.0.push(self.speeds.0);
                self.histories.1.push(self.speeds.1);
            }
        }
        self.before = Some((totals, now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_counts_into_speeds() {
        let mut flow = Flow::default();
        let start = Instant::now();
        flow.update((1000, 5000), start);
        assert!(flow.histories.0.values().is_empty());
        flow.update((5000, 5000), start + 2 * SECOND);
        assert_eq!(flow.speeds, (2000.0, 0.0));
        flow.update((10, 6000), start + 4 * SECOND);
        assert_eq!(flow.speeds, (0.0, 500.0));
        assert_eq!(flow.histories.0.values(), [2000.0, 0.0]);
    }

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn a_spike_says_nothing() {
        let mut watch = Watch::default();
        let start = Instant::now();
        let limits = Limits::default();
        assert_eq!(watch.update(99.0, limits, 10 * SECOND, start), None);
        assert_eq!(
            watch.update(20.0, limits, 10 * SECOND, start + 2 * SECOND),
            None
        );
        assert_eq!(
            watch.update(99.0, limits, 10 * SECOND, start + 4 * SECOND),
            None
        );
        assert!(!watch.critical);
    }

    #[test]
    fn says_once_while_high() {
        let mut watch = Watch::default();
        let start = Instant::now();
        let limits = Limits::default();
        let at = |seconds: u64| start + SECOND * seconds as u32;
        assert_eq!(watch.update(90.0, limits, 10 * SECOND, at(0)), None);
        assert_eq!(
            watch.update(90.0, limits, 10 * SECOND, at(10)),
            Some(Change::High)
        );
        assert_eq!(watch.update(90.0, limits, 10 * SECOND, at(20)), None);
        // Hovering under the level isn't down yet.
        assert_eq!(watch.update(80.0, limits, 10 * SECOND, at(22)), None);
        assert_eq!(watch.update(90.0, limits, 10 * SECOND, at(24)), None);
        assert_eq!(watch.update(90.0, limits, 10 * SECOND, at(40)), None);
        // Well down, then up again: it says so again.
        watch.update(50.0, limits, 10 * SECOND, at(42));
        watch.update(90.0, limits, 10 * SECOND, at(44));
        assert_eq!(
            watch.update(90.0, limits, 10 * SECOND, at(54)),
            Some(Change::High)
        );
    }

    #[test]
    fn critical_shows_until_it_calms() {
        let mut watch = Watch::default();
        let start = Instant::now();
        let limits = Limits::default();
        let at = |seconds: u64| start + SECOND * seconds as u32;
        watch.update(97.0, limits, 5 * SECOND, at(0));
        assert_eq!(
            watch.update(97.0, limits, 5 * SECOND, at(5)),
            Some(Change::Critical)
        );
        assert!(watch.critical);
        assert_eq!(watch.update(90.0, limits, 5 * SECOND, at(7)), None);
        assert_eq!(
            watch.update(80.0, limits, 5 * SECOND, at(9)),
            Some(Change::Calm)
        );
        // The notice level doesn't say anything on the way down.
        assert_eq!(watch.update(88.0, limits, 5 * SECOND, at(20)), None);
    }

    #[test]
    fn zero_turns_a_level_off() {
        let mut watch = Watch::default();
        let start = Instant::now();
        let off = Limits {
            notice: 0,
            critical: 0,
        };
        for seconds in 0..30 {
            assert_eq!(
                watch.update(100.0, off, SECOND, start + SECOND * seconds as u32),
                None
            );
        }
    }

    #[test]
    fn keeps_the_last_readings() {
        let mut history = History::default();
        for value in 0..70 {
            history.push(f64::from(value));
        }
        let values = history.values();
        assert_eq!(values.len(), CAPACITY);
        assert_eq!(values[0], 10.0);
    }
}

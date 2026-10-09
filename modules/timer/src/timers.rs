//! Custom timers, like a kitchen timer's: several at once, each with a
//! length, an optional label, and its own pause. They run beside the focus
//! session without touching it. As with the focus timer, every call takes
//! the wall clock in milliseconds since the epoch.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::clock::clock;

/// The most timers at once.
pub const MOST: usize = 20;
/// How long the daemon waits at most between two looks at the clock, as
/// for the focus timer.
const LOOK: u64 = 1000;

/// One custom timer, counting down or paused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Custom {
    /// Its number, for `pause 2` and the like.
    pub id: u32,
    /// What it's for, like "Tea"; empty without one.
    #[serde(default)]
    pub label: String,
    /// Its whole length, with the time added since it started.
    pub total_ms: u64,
    /// When it runs out, while it runs. `None` while paused.
    pub ends_ms: Option<u64>,
    /// What was left when it paused.
    pub left_ms: u64,
}

impl Custom {
    pub fn paused(&self) -> bool {
        self.ends_ms.is_none()
    }

    pub fn left(&self, now: u64) -> u64 {
        match self.ends_ms {
            Some(ends) => ends.saturating_sub(now),
            None => self.left_ms,
        }
    }

    /// Its label, or what to call it without one.
    pub fn name(&self) -> String {
        if self.label.is_empty() {
            format!("Timer {}", self.id)
        } else {
            self.label.clone()
        }
    }

    fn pause(&mut self, now: u64) {
        if let Some(ends) = self.ends_ms.take() {
            self.left_ms = ends.saturating_sub(now);
        }
    }

    fn resume(&mut self, now: u64) {
        if self.ends_ms.is_none() {
            self.ends_ms = Some(now.saturating_add(self.left_ms));
        }
    }
}

/// Which timers an action means: one by its number, or all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    One(u32),
    All,
}

impl Which {
    /// A number from `list`, or `all`.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text == "all" {
            return Ok(Self::All);
        }
        text.parse()
            .map(Self::One)
            .map_err(|_| format!("{text:?} isn't a timer's number from list, or all"))
    }
}

/// The custom timers, in the order they started.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Timers {
    pub list: Vec<Custom>,
}

impl Timers {
    /// Starts a timer of `ms` from `now`, and returns its number: one more
    /// than the highest running, so they start from 1 again once all end.
    pub fn add(&mut self, ms: u64, label: &str, now: u64) -> Result<u32, String> {
        if self.list.len() >= MOST {
            return Err(format!("{MOST} timers run at most"));
        }
        let id = self.list.iter().map(|timer| timer.id).max().unwrap_or(0) + 1;
        self.list.push(Custom {
            id,
            label: label.to_owned(),
            total_ms: ms,
            ends_ms: Some(now.saturating_add(ms)),
            left_ms: ms,
        });
        Ok(id)
    }

    #[cfg(test)]
    pub fn get(&self, id: u32) -> Option<&Custom> {
        self.list.iter().find(|timer| timer.id == id)
    }

    /// The timers `which` means, or an error when there's none.
    fn pick(&mut self, which: Which) -> Result<Vec<&mut Custom>, String> {
        let picked: Vec<&mut Custom> = self
            .list
            .iter_mut()
            .filter(|timer| match which {
                Which::One(id) => timer.id == id,
                Which::All => true,
            })
            .collect();
        match which {
            _ if !picked.is_empty() => Ok(picked),
            Which::One(id) => Err(format!("no timer {id}; list shows them")),
            Which::All => Err("no timers run".to_owned()),
        }
    }

    pub fn pause(&mut self, which: Which, now: u64) -> Result<(), String> {
        self.pick(which)?
            .into_iter()
            .for_each(|timer| timer.pause(now));
        Ok(())
    }

    pub fn resume(&mut self, which: Which, now: u64) -> Result<(), String> {
        self.pick(which)?
            .into_iter()
            .for_each(|timer| timer.resume(now));
        Ok(())
    }

    /// Pauses a running timer or resumes a paused one.
    pub fn toggle(&mut self, id: u32, now: u64) -> Result<(), String> {
        for timer in self.pick(Which::One(id))? {
            if timer.paused() {
                timer.resume(now);
            } else {
                timer.pause(now);
            }
        }
        Ok(())
    }

    pub fn stop(&mut self, which: Which) -> Result<(), String> {
        self.pick(which)?;
        self.list.retain(|timer| match which {
            Which::One(id) => timer.id != id,
            Which::All => false,
        });
        Ok(())
    }

    /// Adds `ms` to a timer, like a kitchen timer's "+1 min".
    pub fn extend(&mut self, which: Which, ms: u64) -> Result<(), String> {
        for timer in self.pick(which)? {
            if timer.total_ms.saturating_add(ms) > crate::duration::LONGEST_MS {
                return Err("a timer runs a day at most".to_owned());
            }
            timer.total_ms = timer.total_ms.saturating_add(ms);
            match &mut timer.ends_ms {
                Some(ends) => *ends = ends.saturating_add(ms),
                None => timer.left_ms = timer.left_ms.saturating_add(ms),
            }
        }
        Ok(())
    }

    /// Takes out the timers that ran out by `now`, in the order they ran
    /// out.
    pub fn tick(&mut self, now: u64) -> Vec<Custom> {
        let (mut ended, running): (Vec<Custom>, Vec<Custom>) = std::mem::take(&mut self.list)
            .into_iter()
            .partition(|timer| !timer.paused() && timer.left(now) == 0);
        self.list = running;
        ended.sort_by_key(|timer| timer.ends_ms);
        ended
    }

    /// How long to wait before the next `tick`, while one runs.
    pub fn wait(&self, now: u64) -> Option<Duration> {
        self.list
            .iter()
            .filter(|timer| !timer.paused())
            .map(|timer| timer.left(now).clamp(1, LOOK))
            .min()
            .map(Duration::from_millis)
    }

    /// The one that runs out first, paused ones last.
    pub fn soonest(&self, now: u64) -> Option<&Custom> {
        self.list
            .iter()
            .min_by_key(|timer| (timer.paused(), timer.left(now)))
    }

    /// A line each for `mochi ipc timer list`.
    pub fn status(&self, now: u64) -> String {
        if self.list.is_empty() {
            return "no timers".to_owned();
        }
        self.list
            .iter()
            .map(|timer| {
                let paused = if timer.paused() { ", paused" } else { "" };
                format!(
                    "{} {} {} left{paused}",
                    timer.id,
                    timer.name(),
                    clock(timer.left(now))
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: u64 = 60_000;
    const NOW: u64 = 1_790_000_000_000;

    #[test]
    fn several_run_at_once_and_end_in_order() {
        let mut timers = Timers::default();
        assert_eq!(timers.add(10 * MINUTE, "Pizza", NOW), Ok(1));
        assert_eq!(timers.add(3 * MINUTE, "Eggs", NOW), Ok(2));
        assert_eq!(timers.add(MINUTE, "", NOW + MINUTE), Ok(3));
        assert_eq!(
            timers.status(NOW + MINUTE),
            "1 Pizza 9:00 left\n2 Eggs 2:00 left\n3 Timer 3 1:00 left"
        );
        assert_eq!(timers.soonest(NOW + MINUTE).unwrap().id, 3);
        assert_eq!(timers.tick(NOW + MINUTE), []);

        // Asleep through two ends, the one that ran out first comes first.
        let ended = timers.tick(NOW + 5 * MINUTE);
        let names: Vec<String> = ended.iter().map(Custom::name).collect();
        assert_eq!(names, ["Timer 3", "Eggs"]);
        assert_eq!(timers.list.len(), 1);
        // A new one takes the next number after the highest running.
        assert_eq!(timers.add(MINUTE, "", NOW), Ok(2));
        timers.stop(Which::All).unwrap();
        assert_eq!(timers.add(MINUTE, "", NOW), Ok(1));
    }

    #[test]
    fn each_pauses_on_its_own() {
        let mut timers = Timers::default();
        timers.add(10 * MINUTE, "Pizza", NOW).unwrap();
        timers.add(5 * MINUTE, "Tea", NOW).unwrap();
        timers.pause(Which::One(1), NOW + MINUTE).unwrap();
        assert_eq!(timers.wait(NOW + MINUTE), Some(Duration::from_secs(1)));
        let ended = timers.tick(NOW + 20 * MINUTE);
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].label, "Tea");
        // Paused, it kept its 9 minutes, and runs on from there.
        assert_eq!(
            timers.status(NOW + 20 * MINUTE),
            "1 Pizza 9:00 left, paused"
        );
        assert_eq!(timers.wait(NOW + 20 * MINUTE), None);
        timers.toggle(1, NOW + 20 * MINUTE).unwrap();
        assert_eq!(timers.get(1).unwrap().ends_ms, Some(NOW + 29 * MINUTE));
        timers.toggle(1, NOW + 21 * MINUTE).unwrap();
        assert!(timers.get(1).unwrap().paused());
        timers.resume(Which::All, NOW + 22 * MINUTE).unwrap();
        assert_eq!(timers.get(1).unwrap().ends_ms, Some(NOW + 30 * MINUTE));
    }

    #[test]
    fn a_minute_more_running_or_paused() {
        let mut timers = Timers::default();
        timers.add(MINUTE, "", NOW).unwrap();
        timers.add(MINUTE, "", NOW).unwrap();
        timers.extend(Which::One(1), MINUTE).unwrap();
        assert_eq!(timers.get(1).unwrap().ends_ms, Some(NOW + 2 * MINUTE));
        assert_eq!(timers.get(1).unwrap().total_ms, 2 * MINUTE);
        timers.pause(Which::One(2), NOW).unwrap();
        timers.extend(Which::One(2), MINUTE).unwrap();
        assert_eq!(timers.get(2).unwrap().left_ms, 2 * MINUTE);
        assert!(timers.extend(Which::One(1), 24 * 60 * MINUTE).is_err());
    }

    #[test]
    fn says_when_there_is_no_such_timer() {
        let mut timers = Timers::default();
        assert_eq!(timers.pause(Which::All, NOW), Err("no timers run".into()));
        timers.add(MINUTE, "", NOW).unwrap();
        assert!(
            timers
                .stop(Which::One(7))
                .unwrap_err()
                .contains("no timer 7")
        );
        assert_eq!(Which::parse("2"), Ok(Which::One(2)));
        assert_eq!(Which::parse("all"), Ok(Which::All));
        assert!(Which::parse("Tea").is_err());
        for _ in 1..MOST {
            timers.add(MINUTE, "", NOW).unwrap();
        }
        assert!(timers.add(MINUTE, "", NOW).is_err());
    }

    #[test]
    fn saved_timers_load_back() {
        let mut timers = Timers::default();
        timers.add(MINUTE, "Tea", NOW).unwrap();
        timers.add(MINUTE, "", NOW).unwrap();
        timers.pause(Which::One(2), NOW).unwrap();
        let text = serde_json::to_string(&timers).unwrap();
        assert_eq!(serde_json::from_str::<Timers>(&text).unwrap(), timers);
    }
}

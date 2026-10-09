//! The timer without the daemon: the phase counting down, what's left of it,
//! and what comes when it runs out. Every call takes the time as
//! milliseconds since the epoch, so the tests don't wait.
//!
//! The time is the wall clock's, not a monotonic one, so a phase that ran out
//! while the computer slept ends as soon as it wakes, and the views count
//! down from the same end without asking the daemon every second.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::Settings;

/// How long the daemon waits at most between two looks at the clock while a
/// phase runs, so the end of one that ran out during a suspend comes soon
/// after waking.
const LOOK: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Focus,
    Break,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Break => "break",
        }
    }
}

/// One phase counting down, or paused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Countdown {
    pub phase: Phase,
    /// The whole phase, in milliseconds.
    pub total_ms: u64,
    /// When it runs out, while it runs. `None` while paused.
    pub ends_ms: Option<u64>,
    /// What was left when it paused.
    pub left_ms: u64,
}

impl Countdown {
    pub fn paused(&self) -> bool {
        self.ends_ms.is_none()
    }

    pub fn left(&self, now: u64) -> u64 {
        match self.ends_ms {
            Some(ends) => ends.saturating_sub(now),
            None => self.left_ms,
        }
    }
}

/// The timer: the phase counting down, if any, and the focus sessions
/// finished so far.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clock {
    pub countdown: Option<Countdown>,
    pub sessions: u32,
}

/// A phase that ran out, and what comes next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ended {
    pub finished: Phase,
    pub next: Phase,
    /// How long the next phase is.
    pub minutes: u64,
    /// The next phase is a long break.
    pub long: bool,
    /// The break started by itself, with `auto_break`.
    pub started: bool,
}

impl Clock {
    /// Starts `phase` for `minutes` from `now`, in place of what ran.
    pub fn start(&mut self, phase: Phase, minutes: u64, now: u64) {
        let total = minutes.max(1).saturating_mul(60_000);
        self.countdown = Some(Countdown {
            phase,
            total_ms: total,
            ends_ms: Some(now.saturating_add(total)),
            left_ms: total,
        });
    }

    pub fn stop(&mut self) {
        self.countdown = None;
    }

    /// Pauses what runs. Pausing a paused timer changes nothing.
    pub fn pause(&mut self, now: u64) -> Result<(), String> {
        let countdown = self.countdown.as_mut().ok_or("the timer isn't running")?;
        if let Some(ends) = countdown.ends_ms.take() {
            countdown.left_ms = ends.saturating_sub(now);
        }
        Ok(())
    }

    /// Carries on from where it paused. Resuming a running timer changes
    /// nothing.
    pub fn resume(&mut self, now: u64) -> Result<(), String> {
        let countdown = self.countdown.as_mut().ok_or("the timer isn't running")?;
        if countdown.ends_ms.is_none() {
            countdown.ends_ms = Some(now.saturating_add(countdown.left_ms));
        }
        Ok(())
    }

    /// Pauses or resumes, or starts a focus session when nothing runs.
    pub fn toggle(&mut self, now: u64, settings: &Settings) {
        match &self.countdown {
            Some(countdown) if countdown.paused() => {
                let _ = self.resume(now);
            }
            Some(_) => {
                let _ = self.pause(now);
            }
            None => self.start(Phase::Focus, settings.focus_minutes, now),
        }
    }

    /// How long the next break is, and whether it's a long one: after every
    /// `sessions_before_long_break` focus sessions.
    pub fn next_break(&self, settings: &Settings) -> (u64, bool) {
        let every = settings.sessions_before_long_break;
        let long = every > 0 && self.sessions > 0 && self.sessions.is_multiple_of(every);
        let minutes = if long {
            settings.long_break_minutes
        } else {
            settings.break_minutes
        };
        (minutes, long)
    }

    /// Ends the phase that ran out by `now`, if one did: a focus session
    /// counts, and its break starts with `auto_break`.
    pub fn tick(&mut self, now: u64, settings: &Settings) -> Option<Ended> {
        let countdown = self.countdown.as_ref()?;
        if countdown.paused() || countdown.left(now) > 0 {
            return None;
        }
        let finished = countdown.phase;
        self.countdown = None;
        let ended = match finished {
            Phase::Focus => {
                self.sessions += 1;
                let (minutes, long) = self.next_break(settings);
                if settings.auto_break {
                    self.start(Phase::Break, minutes, now);
                }
                Ended {
                    finished,
                    next: Phase::Break,
                    minutes,
                    long,
                    started: settings.auto_break,
                }
            }
            Phase::Break => Ended {
                finished,
                next: Phase::Focus,
                minutes: settings.focus_minutes,
                long: false,
                started: false,
            },
        };
        Some(ended)
    }

    /// How long to wait before the next `tick`, while a phase runs.
    pub fn wait(&self, now: u64) -> Option<Duration> {
        let countdown = self.countdown.as_ref().filter(|c| !c.paused())?;
        Some(Duration::from_millis(countdown.left(now).clamp(1, LOOK)))
    }

    /// One line for `mochi ipc timer status`.
    pub fn status(&self, now: u64) -> String {
        match &self.countdown {
            Some(countdown) => {
                let paused = if countdown.paused() { ", paused" } else { "" };
                format!(
                    "{} {} left{paused}",
                    countdown.phase.name(),
                    clock(countdown.left(now))
                )
            }
            None if self.sessions == 1 => "idle, 1 session done".to_owned(),
            None => format!("idle, {} sessions done", self.sessions),
        }
    }
}

/// Milliseconds as m:ss, or h:mm:ss from an hour, rounded up so the last
/// second shows 0:01 rather than 0:00.
pub fn clock(ms: u64) -> String {
    let seconds = ms.div_ceil(1000);
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: u64 = 60_000;
    /// Some moment, as milliseconds since the epoch.
    const NOW: u64 = 1_790_000_000_000;

    #[test]
    fn a_focus_session_counts_down_and_pauses() {
        let settings = Settings::default();
        let mut clock = Clock::default();
        clock.start(Phase::Focus, 25, NOW);
        assert_eq!(clock.status(NOW), "focus 25:00 left");
        assert_eq!(clock.status(NOW + 10 * MINUTE + 1), "focus 15:00 left");

        // Paused, the time stands still.
        clock.pause(NOW + 10 * MINUTE).unwrap();
        assert_eq!(clock.wait(NOW + 10 * MINUTE), None);
        assert_eq!(clock.tick(NOW + 60 * MINUTE, &settings), None);
        assert_eq!(clock.status(NOW + 60 * MINUTE), "focus 15:00 left, paused");

        // Resumed an hour later, 15 minutes are still left.
        clock.resume(NOW + 70 * MINUTE).unwrap();
        assert_eq!(clock.tick(NOW + 84 * MINUTE, &settings), None);
        let ended = clock.tick(NOW + 85 * MINUTE, &settings).unwrap();
        assert_eq!(
            ended,
            Ended {
                finished: Phase::Focus,
                next: Phase::Break,
                minutes: 5,
                long: false,
                started: false,
            }
        );
        assert_eq!(clock.countdown, None);
        assert_eq!(clock.sessions, 1);
        assert_eq!(clock.status(NOW + 85 * MINUTE), "idle, 1 session done");
    }

    #[test]
    fn auto_break_starts_the_break_and_its_end_says_focus_is_next() {
        let settings = Settings {
            auto_break: true,
            ..Settings::default()
        };
        let mut clock = Clock::default();
        clock.start(Phase::Focus, 25, NOW);
        // The computer slept through the end: the break starts on waking.
        let ended = clock.tick(NOW + 40 * MINUTE, &settings).unwrap();
        assert!(ended.started);
        let countdown = clock.countdown.clone().unwrap();
        assert_eq!(countdown.phase, Phase::Break);
        assert_eq!(countdown.ends_ms, Some(NOW + 45 * MINUTE));

        let ended = clock.tick(NOW + 45 * MINUTE, &settings).unwrap();
        assert_eq!((ended.finished, ended.next), (Phase::Break, Phase::Focus));
        assert_eq!((ended.minutes, ended.started), (25, false));
        assert_eq!(clock.countdown, None);
        // A break isn't a session.
        assert_eq!(clock.sessions, 1);
    }

    #[test]
    fn every_fourth_break_is_long() {
        let settings = Settings::default();
        let mut clock = Clock::default();
        let mut breaks = Vec::new();
        for session in 0..8 {
            let start = NOW + session * 30 * MINUTE;
            clock.start(Phase::Focus, 25, start);
            let ended = clock.tick(start + 25 * MINUTE, &settings).unwrap();
            breaks.push((ended.minutes, ended.long));
        }
        let short = (5, false);
        let long = (15, true);
        assert_eq!(
            breaks,
            [short, short, short, long, short, short, short, long]
        );

        let never = Settings {
            sessions_before_long_break: 0,
            ..Settings::default()
        };
        assert_eq!(clock.next_break(&never), short);
    }

    #[test]
    fn toggle_starts_pauses_and_resumes() {
        let settings = Settings {
            focus_minutes: 50,
            ..Settings::default()
        };
        let mut clock = Clock::default();
        assert!(clock.pause(NOW).is_err());
        assert!(clock.resume(NOW).is_err());

        clock.toggle(NOW, &settings);
        assert_eq!(clock.status(NOW), "focus 50:00 left");
        clock.toggle(NOW + MINUTE, &settings);
        assert!(clock.countdown.as_ref().unwrap().paused());
        clock.toggle(NOW + 2 * MINUTE, &settings);
        assert_eq!(clock.status(NOW + 2 * MINUTE), "focus 49:00 left");
        // Pausing twice keeps the first pause's time.
        clock.pause(NOW + 3 * MINUTE).unwrap();
        clock.pause(NOW + 9 * MINUTE).unwrap();
        assert_eq!(clock.status(NOW + 9 * MINUTE), "focus 48:00 left, paused");

        clock.stop();
        assert_eq!(clock.status(NOW), "idle, 0 sessions done");
    }

    #[test]
    fn the_wait_ends_at_the_end_or_within_a_second() {
        let mut clock = Clock::default();
        assert_eq!(clock.wait(NOW), None);
        clock.start(Phase::Break, 5, NOW);
        assert_eq!(clock.wait(NOW), Some(Duration::from_secs(1)));
        assert_eq!(
            clock.wait(NOW + 5 * MINUTE - 300),
            Some(Duration::from_millis(300))
        );
        assert_eq!(clock.wait(NOW + 6 * MINUTE), Some(Duration::from_millis(1)));
    }

    #[test]
    fn times_read_as_a_clock() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(1), "0:01");
        assert_eq!(clock(59_001), "1:00");
        assert_eq!(clock(25 * MINUTE), "25:00");
        assert_eq!(clock(90 * MINUTE + 5_000), "1:30:05");
    }

    #[test]
    fn a_saved_clock_loads_back() {
        let mut clock = Clock {
            sessions: 3,
            ..Clock::default()
        };
        clock.start(Phase::Focus, 25, NOW);
        clock.pause(NOW + MINUTE).unwrap();
        let text = serde_json::to_string(&clock).unwrap();
        assert_eq!(serde_json::from_str::<Clock>(&text).unwrap(), clock);
        // An empty or older file is an idle timer.
        assert_eq!(
            serde_json::from_str::<Clock>("{}").unwrap(),
            Clock::default()
        );
    }
}

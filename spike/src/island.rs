//! A minimal stand-in for the arbiter: one current activity, a timeout back
//! to idle, and hover pausing that timeout. The real arbiter adds priorities
//! and a queue.

use std::time::{Duration, Instant};

use crate::protocol::Activity;

/// After the pointer leaves, an activity stays at least this long.
const MIN_AFTER_HOVER: Duration = Duration::from_secs(1);

pub struct Island {
    next_id: u64,
    current: Activity,
    deadline: Option<Instant>,
    /// Time left on the timeout while the pointer is over the island.
    paused: Option<Duration>,
}

impl Island {
    pub fn new() -> Self {
        let mut island = Self {
            next_id: 0,
            current: Activity {
                id: 0,
                module: String::new(),
                view: String::new(),
                payload: serde_json::Value::Null,
            },
            deadline: None,
            paused: None,
        };
        island.show_idle();
        island
    }

    pub fn current(&self) -> &Activity {
        &self.current
    }

    pub fn is_idle(&self) -> bool {
        self.current.module == "idle"
    }

    pub fn show_idle(&mut self) {
        self.present("idle", "Pill", serde_json::json!({}), None, Instant::now());
    }

    pub fn present(
        &mut self,
        module: &str,
        view: &str,
        payload: serde_json::Value,
        timeout: Option<Duration>,
        now: Instant,
    ) {
        self.next_id += 1;
        self.current = Activity {
            id: self.next_id,
            module: module.to_owned(),
            view: view.to_owned(),
            payload,
        };
        self.deadline = timeout.map(|timeout| now + timeout);
        self.paused = None;
    }

    pub fn set_hovered(&mut self, hovered: bool, now: Instant) {
        if hovered {
            if let Some(deadline) = self.deadline.take() {
                self.paused = Some(deadline.saturating_duration_since(now));
            }
        } else if let Some(remaining) = self.paused.take() {
            self.deadline = Some(now + remaining.max(MIN_AFTER_HOVER));
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Returns to idle if the current activity has timed out. Returns whether
    /// the activity changed.
    pub fn expire(&mut self, now: Instant) -> bool {
        match self.deadline {
            Some(deadline) if deadline <= now => {
                self.show_idle();
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(4);

    fn demo(island: &mut Island, now: Instant) {
        island.present("demo", "Card", serde_json::json!({}), Some(TIMEOUT), now);
    }

    #[test]
    fn starts_idle_without_deadline() {
        let island = Island::new();
        assert!(island.is_idle());
        assert_eq!(island.deadline(), None);
    }

    #[test]
    fn every_activity_gets_a_new_id() {
        let mut island = Island::new();
        let first = island.current().id;
        demo(&mut island, Instant::now());
        assert!(island.current().id > first);
    }

    #[test]
    fn times_out_back_to_idle() {
        let now = Instant::now();
        let mut island = Island::new();
        demo(&mut island, now);

        assert!(!island.expire(now + TIMEOUT - Duration::from_millis(1)));
        assert!(!island.is_idle());
        assert!(island.expire(now + TIMEOUT));
        assert!(island.is_idle());
    }

    #[test]
    fn hover_pauses_the_timeout() {
        let now = Instant::now();
        let mut island = Island::new();
        demo(&mut island, now);

        island.set_hovered(true, now + Duration::from_secs(3));
        assert!(!island.expire(now + Duration::from_secs(60)));

        // One second was left when the pointer arrived.
        let left = now + Duration::from_secs(60);
        island.set_hovered(false, left);
        assert_eq!(island.deadline(), Some(left + Duration::from_secs(1)));
    }

    #[test]
    fn leaving_right_before_the_deadline_keeps_the_minimum() {
        let now = Instant::now();
        let mut island = Island::new();
        demo(&mut island, now);

        island.set_hovered(true, now + TIMEOUT - Duration::from_millis(10));
        island.set_hovered(false, now + TIMEOUT);
        assert_eq!(island.deadline(), Some(now + TIMEOUT + MIN_AFTER_HOVER));
    }
}

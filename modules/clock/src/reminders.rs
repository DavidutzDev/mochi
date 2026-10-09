//! Reminders without the daemon: what's kept in
//! `$XDG_STATE_HOME/mochi/reminders.json`, and which are due. Every call
//! takes the time as seconds since the epoch, so the tests don't wait.
//!
//! A reminder is due from its time until it's marked done; snoozing moves
//! when it's due, not its time. One that came due while mochid was down or
//! the computer slept is due as soon as either is back, since the times are
//! the wall clock's.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The longest a reminder's text may be, in characters.
const LONGEST: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reminder {
    pub id: u64,
    /// When, in seconds since the epoch.
    pub at: i64,
    pub text: String,
    /// Done: it won't come up again. Kept for the calendar.
    #[serde(default)]
    pub done: bool,
    /// When it comes up again after a snooze.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed: Option<i64>,
}

impl Reminder {
    /// When it's due: its time, or the end of a snooze.
    pub fn due_at(&self) -> i64 {
        self.snoozed.unwrap_or(self.at)
    }

    pub fn due(&self, now: i64) -> bool {
        !self.done && self.due_at() <= now
    }
}

/// Every reminder, in order of time, and the id the next one gets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reminders {
    next: u64,
    reminders: Vec<Reminder>,
}

impl Reminders {
    /// `$XDG_STATE_HOME/mochi/reminders.json`.
    pub fn path() -> Option<PathBuf> {
        Some(mochi_core::config::state_dir()?.join("reminders.json"))
    }

    /// What `path` keeps; none when it's missing or broken, which the log
    /// says.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let mut reminders: Self = serde_json::from_str(&text)
            .inspect_err(|error| tracing::warn!(%error, "ignoring the saved reminders"))
            .unwrap_or_default();
        reminders.reminders.sort_by_key(|reminder| reminder.at);
        // A file edited by hand may have ids past `next`.
        let highest = reminders.reminders.iter().map(|reminder| reminder.id).max();
        reminders.next = reminders.next.max(highest.map_or(0, |id| id + 1));
        reminders
    }

    /// Writes them to a file next to `path`, then puts it in its place, so
    /// a crash halfway leaves the old ones.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let temporary = path.with_extension("json.new");
        path.parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::write(
                    &temporary,
                    serde_json::to_vec_pretty(self).unwrap_or_default(),
                )
            })
            .and_then(|()| std::fs::rename(&temporary, path))
            .map_err(|error| format!("can't save the reminders: {error}"))
    }

    pub fn all(&self) -> &[Reminder] {
        &self.reminders
    }

    #[cfg(test)]
    pub fn get(&self, id: u64) -> Option<&Reminder> {
        self.reminders.iter().find(|reminder| reminder.id == id)
    }

    /// Adds one at `at`, which mustn't have passed, and returns its id.
    pub fn add(&mut self, at: i64, text: &str, now: i64) -> Result<u64, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("a reminder needs some text".to_owned());
        }
        if text.chars().count() > LONGEST {
            return Err(format!("a reminder takes up to {LONGEST} characters"));
        }
        // The minute that runs is fine: it comes up at once.
        if at < now - 60 {
            return Err("that time has passed".to_owned());
        }
        let id = self.next;
        self.next += 1;
        let place = self.reminders.partition_point(|other| other.at <= at);
        self.reminders.insert(
            place,
            Reminder {
                id,
                at,
                text: text.to_owned(),
                done: false,
                snoozed: None,
            },
        );
        Ok(id)
    }

    pub fn delete(&mut self, id: u64) -> Result<Reminder, String> {
        let place = self.place(id)?;
        Ok(self.reminders.remove(place))
    }

    /// Marks one done: it won't come up again.
    pub fn done(&mut self, id: u64) -> Result<(), String> {
        let place = self.place(id)?;
        let reminder = &mut self.reminders[place];
        reminder.done = true;
        reminder.snoozed = None;
        Ok(())
    }

    /// Brings one up again at `until`.
    pub fn snooze(&mut self, id: u64, until: i64) -> Result<(), String> {
        let place = self.place(id)?;
        let reminder = &mut self.reminders[place];
        if reminder.done {
            return Err(format!("reminder {id} is done"));
        }
        reminder.snoozed = Some(until);
        Ok(())
    }

    /// The ones due now, earliest first.
    pub fn due(&self, now: i64) -> impl Iterator<Item = &Reminder> {
        self.reminders
            .iter()
            .filter(move |reminder| reminder.due(now))
    }

    /// When the next one comes due after `now`, if any does.
    pub fn next_due(&self, now: i64) -> Option<i64> {
        self.reminders
            .iter()
            .filter(|reminder| !reminder.done)
            .map(Reminder::due_at)
            .filter(|at| *at > now)
            .min()
    }

    fn place(&self, id: u64) -> Result<usize, String> {
        self.reminders
            .iter()
            .position(|reminder| reminder.id == id)
            .ok_or_else(|| format!("there's no reminder {id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Some moment, in seconds since the epoch.
    const NOW: i64 = 1_790_000_000;
    const HOUR: i64 = 3600;

    #[test]
    fn come_due_at_their_time_until_done() {
        let mut reminders = Reminders::default();
        let later = reminders.add(NOW + 2 * HOUR, "Call Ana", NOW).unwrap();
        let sooner = reminders
            .add(NOW + HOUR, "Take the bread out", NOW)
            .unwrap();
        // In order of time, whatever the order they came in.
        let texts: Vec<&str> = reminders.all().iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["Take the bread out", "Call Ana"]);

        assert_eq!(reminders.due(NOW).count(), 0);
        assert_eq!(reminders.next_due(NOW), Some(NOW + HOUR));
        let due: Vec<u64> = reminders.due(NOW + HOUR).map(|r| r.id).collect();
        assert_eq!(due, [sooner]);
        // Still due hours later: a reminder waits for you.
        let due: Vec<u64> = reminders.due(NOW + 5 * HOUR).map(|r| r.id).collect();
        assert_eq!(due, [sooner, later]);

        reminders.done(sooner).unwrap();
        let due: Vec<u64> = reminders.due(NOW + 5 * HOUR).map(|r| r.id).collect();
        assert_eq!(due, [later]);
        assert_eq!(reminders.next_due(NOW + HOUR), Some(NOW + 2 * HOUR));
        reminders.done(later).unwrap();
        assert_eq!(reminders.next_due(NOW), None);
    }

    #[test]
    fn a_snooze_moves_when_it_is_due() {
        let mut reminders = Reminders::default();
        let id = reminders.add(NOW, "Stretch", NOW).unwrap();
        assert_eq!(reminders.due(NOW).count(), 1);
        reminders.snooze(id, NOW + 600).unwrap();
        assert_eq!(reminders.due(NOW + 599).count(), 0);
        assert_eq!(reminders.next_due(NOW), Some(NOW + 600));
        assert_eq!(reminders.due(NOW + 600).count(), 1);
        // Its time stays, for the calendar.
        assert_eq!(reminders.get(id).unwrap().at, NOW);
        reminders.done(id).unwrap();
        assert!(reminders.snooze(id, NOW + 1200).is_err());
    }

    #[test]
    fn refuses_what_makes_no_reminder() {
        let mut reminders = Reminders::default();
        assert!(reminders.add(NOW + HOUR, "   ", NOW).is_err());
        assert!(reminders.add(NOW - HOUR, "Too late", NOW).is_err());
        assert!(reminders.add(NOW + HOUR, &"a".repeat(201), NOW).is_err());
        // This minute is still fine.
        assert!(reminders.add(NOW - 30, "Now", NOW).is_ok());
        assert!(reminders.delete(7).is_err());
        assert!(reminders.done(7).is_err());
    }

    #[test]
    fn deleting_keeps_ids_apart() {
        let mut reminders = Reminders::default();
        let first = reminders.add(NOW + HOUR, "One", NOW).unwrap();
        assert_eq!(reminders.delete(first).unwrap().text, "One");
        let second = reminders.add(NOW + HOUR, "Two", NOW).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn a_restart_finds_them_due() {
        let path = std::env::temp_dir().join(format!(
            "mochi-reminders-{}/reminders.json",
            std::process::id()
        ));
        let mut reminders = Reminders::default();
        let id = reminders.add(NOW + HOUR, "Water the plants", NOW).unwrap();
        reminders.add(NOW + 2 * HOUR, "Done already", NOW).unwrap();
        reminders.done(1).unwrap();
        reminders.save(&path).unwrap();

        // Read again after the time went by, as after a restart or a
        // suspend.
        let mut loaded = Reminders::load(&path);
        assert_eq!(loaded, reminders);
        let due: Vec<u64> = loaded.due(NOW + 3 * HOUR).map(|r| r.id).collect();
        assert_eq!(due, [id]);
        // New ones don't take an old id.
        assert_eq!(loaded.add(NOW + HOUR, "Next", NOW).unwrap(), 2);

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Reminders::load(&path), Reminders::default());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert_eq!(Reminders::load(&path), Reminders::default());
    }

    #[test]
    fn a_file_edited_by_hand_reads() {
        let path =
            std::env::temp_dir().join(format!("mochi-reminders-hand-{}.json", std::process::id()));
        std::fs::write(
            &path,
            r#"{ "reminders": [
                { "id": 9, "at": 1790003600, "text": "Later" },
                { "id": 4, "at": 1790000000, "text": "Sooner", "done": true }
            ] }"#,
        )
        .unwrap();
        let mut loaded = Reminders::load(&path);
        assert_eq!(loaded.all()[0].text, "Sooner");
        assert_eq!(loaded.add(NOW + HOUR, "New", NOW).unwrap(), 10);
        std::fs::remove_file(path).unwrap();
    }
}

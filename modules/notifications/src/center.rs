//! Where each notification is: popping up on the island, or waiting in the
//! history. Never talks to anything; every change returns the [`Effect`]s the
//! module carries out.
//!
//! - A new notification pops up, unless do not disturb is on and it isn't
//!   critical; then it goes straight to the history.
//! - With `replace_same_app`, a new popup takes the place of the popup from
//!   the same app, which goes to the history. Critical ones always get their
//!   own.
//! - A popup that runs out of time goes to the history, unless it's
//!   transient. One the user closes is gone.
//! - A notification that replaces another (`replaces_id`) updates it where it
//!   is.
//! - The history keeps the newest `history_max`; older ones close.

use std::collections::{BTreeMap, BTreeSet};

use crate::note::{Note, Urgency};

/// Why a notification closed, as the spec numbers it for
/// `NotificationClosed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Expired = 1,
    Dismissed = 2,
    /// The app called `CloseNotification`.
    Closed = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Show or update this notification's popup.
    Popup(u32),
    /// Take this notification's popup down.
    Withdraw(u32),
    /// Tell the app, and forget anything kept for it, like its image.
    Closed(u32, Reason),
    /// Tell the app the user picked this action.
    Invoked(u32, String),
    /// This notification's popup was taken over by the next `Popup` from the
    /// same app, which replaces it in place.
    Superseded(u32),
}

/// How a popup left the island.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupEnd {
    TimedOut,
    Dismissed,
}

#[derive(Debug)]
pub struct Center {
    notes: BTreeMap<u32, Note>,
    popups: BTreeSet<u32>,
    /// Oldest first.
    history: Vec<u32>,
    history_max: usize,
    replace_same_app: bool,
    dnd: bool,
}

impl Center {
    pub fn new(history_max: usize, replace_same_app: bool) -> Self {
        Self {
            notes: BTreeMap::new(),
            popups: BTreeSet::new(),
            history: Vec::new(),
            history_max,
            replace_same_app,
            dnd: false,
        }
    }

    pub fn get(&self, id: u32) -> Option<&Note> {
        self.notes.get(&id)
    }

    /// Newest first.
    pub fn history(&self) -> impl Iterator<Item = &Note> {
        self.history
            .iter()
            .rev()
            .filter_map(|id| self.notes.get(id))
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn dnd(&self) -> bool {
        self.dnd
    }

    pub fn set_dnd(&mut self, on: bool) {
        self.dnd = on;
    }

    pub fn notify(&mut self, note: Note) -> Vec<Effect> {
        let id = note.id;
        let quiet = self.dnd && note.urgency < Urgency::Critical;
        let known = self.notes.insert(id, note).is_some();
        if self.popups.contains(&id) {
            return vec![Effect::Popup(id)];
        }
        if known {
            // Updating a missed notification doesn't bring it back up.
            return Vec::new();
        }
        if quiet {
            return self.keep(id);
        }
        let mut effects = Vec::new();
        for older in self.same_app_popups(id) {
            self.popups.remove(&older);
            effects.push(Effect::Superseded(older));
            effects.extend(self.keep(older));
        }
        self.popups.insert(id);
        effects.push(Effect::Popup(id));
        effects
    }

    /// Whether this notification's popup replaces the one from the same app.
    pub fn replaces_same_app(&self, note: &Note) -> bool {
        self.replace_same_app && note.urgency < Urgency::Critical && !note.app.is_empty()
    }

    /// The popups a new notification takes the place of.
    fn same_app_popups(&self, id: u32) -> Vec<u32> {
        let Some(note) = self
            .notes
            .get(&id)
            .filter(|note| self.replaces_same_app(note))
        else {
            return Vec::new();
        };
        self.popups
            .iter()
            .copied()
            .filter(|other| {
                self.notes
                    .get(other)
                    .is_some_and(|other| other.app == note.app && self.replaces_same_app(other))
            })
            .collect()
    }

    pub fn popup_ended(&mut self, id: u32, end: PopupEnd) -> Vec<Effect> {
        if !self.popups.remove(&id) {
            return Vec::new();
        }
        let transient = self.notes.get(&id).is_some_and(|note| note.transient);
        match end {
            PopupEnd::TimedOut if !transient => self.keep(id),
            PopupEnd::TimedOut => self.forget(id, Reason::Expired),
            PopupEnd::Dismissed => self.forget(id, Reason::Dismissed),
        }
    }

    /// Closes a notification wherever it is.
    pub fn close(&mut self, id: u32, reason: Reason) -> Vec<Effect> {
        if !self.notes.contains_key(&id) {
            return Vec::new();
        }
        let mut effects = Vec::new();
        if self.popups.remove(&id) {
            effects.push(Effect::Withdraw(id));
        }
        effects.extend(self.forget(id, reason));
        effects
    }

    /// The user picked one of a notification's actions.
    pub fn invoke(&mut self, id: u32, key: &str) -> Result<Vec<Effect>, String> {
        let note = self
            .notes
            .get(&id)
            .ok_or_else(|| format!("no notification {id}"))?;
        if note.action(key).is_none() {
            return Err(format!("notification {id} has no action {key:?}"));
        }
        let resident = note.resident;
        let mut effects = vec![Effect::Invoked(id, key.to_owned())];
        if !resident {
            effects.extend(self.close(id, Reason::Dismissed));
        }
        Ok(effects)
    }

    /// Puts notifications from before a restart back in the history, oldest
    /// first. Past the limit, the oldest go without a word: their apps
    /// aren't listening anymore.
    pub fn restore(&mut self, notes: Vec<Note>) {
        for note in notes {
            if self.notes.contains_key(&note.id) {
                continue;
            }
            self.history.push(note.id);
            self.notes.insert(note.id, note);
        }
        let excess = self.history.len().saturating_sub(self.history_max);
        for id in self.history.drain(..excess).collect::<Vec<_>>() {
            self.notes.remove(&id);
        }
    }

    /// The highest id in use, so new notifications get others.
    pub fn last_id(&self) -> u32 {
        self.notes.keys().next_back().copied().unwrap_or(0)
    }

    /// Empties the history.
    pub fn clear(&mut self) -> Vec<Effect> {
        let ids = std::mem::take(&mut self.history);
        ids.into_iter()
            .flat_map(|id| self.forget(id, Reason::Dismissed))
            .collect()
    }

    /// Moves a notification into the history, closing the oldest past the
    /// limit.
    fn keep(&mut self, id: u32) -> Vec<Effect> {
        self.history.push(id);
        let excess = self.history.len().saturating_sub(self.history_max);
        let dropped: Vec<u32> = self.history.drain(..excess).collect();
        dropped
            .into_iter()
            .flat_map(|id| self.forget(id, Reason::Expired))
            .collect()
    }

    fn forget(&mut self, id: u32, reason: Reason) -> Vec<Effect> {
        self.history.retain(|kept| *kept != id);
        match self.notes.remove(&id) {
            Some(_) => vec![Effect::Closed(id, reason)],
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use crate::note::Action;

    fn note(id: u32, urgency: Urgency) -> Note {
        Note {
            id,
            app: "app".into(),
            icon: String::new(),
            summary: format!("note {id}"),
            body: String::new(),
            actions: vec![Action {
                key: "default".into(),
                label: "Open".into(),
            }],
            urgency,
            timeout: None,
            image: None,
            resident: false,
            transient: false,
            received: SystemTime::UNIX_EPOCH,
        }
    }

    fn history(center: &Center) -> Vec<u32> {
        center.history().map(|note| note.id).collect()
    }

    #[test]
    fn restored_notes_join_the_history_within_the_limit() {
        let mut center = Center::new(2, false);
        center.restore(vec![
            note(4, Urgency::Normal),
            note(5, Urgency::Normal),
            note(9, Urgency::Low),
        ]);
        assert_eq!(history(&center), [9, 5]);
        assert_eq!(center.last_id(), 9);
        assert!(center.get(4).is_none());
        // They close like any other.
        assert_eq!(
            center.close(5, Reason::Dismissed),
            [Effect::Closed(5, Reason::Dismissed)]
        );
    }

    #[test]
    fn popups_go_to_the_history_unless_dismissed() {
        let mut center = Center::new(10, false);
        assert_eq!(center.notify(note(1, Urgency::Normal)), [Effect::Popup(1)]);
        assert_eq!(center.notify(note(2, Urgency::Normal)), [Effect::Popup(2)]);

        assert_eq!(center.popup_ended(1, PopupEnd::TimedOut), []);
        assert_eq!(
            center.popup_ended(2, PopupEnd::Dismissed),
            [Effect::Closed(2, Reason::Dismissed)]
        );
        assert_eq!(history(&center), [1]);
        // A second end for the same popup changes nothing.
        assert_eq!(center.popup_ended(1, PopupEnd::Dismissed), []);
    }

    #[test]
    fn a_new_popup_from_the_same_app_takes_over() {
        let mut center = Center::new(10, true);
        let from = |id, app: &str| Note {
            app: app.into(),
            ..note(id, Urgency::Normal)
        };
        center.notify(from(1, "Discord"));
        center.notify(from(2, "Mail"));
        assert_eq!(
            center.notify(from(3, "Discord")),
            [Effect::Superseded(1), Effect::Popup(3)]
        );
        // The message that was replaced counts as missed.
        assert_eq!(history(&center), [1]);

        // Critical ones neither replace nor get replaced.
        assert_eq!(
            center.notify(Note {
                app: "Discord".into(),
                ..note(4, Urgency::Critical)
            }),
            [Effect::Popup(4)]
        );
        assert_eq!(
            center.notify(from(5, "Discord")),
            [Effect::Superseded(3), Effect::Popup(5)]
        );

        let mut stacking = Center::new(10, false);
        stacking.notify(from(1, "Discord"));
        assert_eq!(stacking.notify(from(2, "Discord")), [Effect::Popup(2)]);
    }

    #[test]
    fn transient_popups_never_reach_the_history() {
        let mut center = Center::new(10, false);
        center.notify(Note {
            transient: true,
            ..note(1, Urgency::Normal)
        });
        assert_eq!(
            center.popup_ended(1, PopupEnd::TimedOut),
            [Effect::Closed(1, Reason::Expired)]
        );
        assert_eq!(history(&center), Vec::<u32>::new());
    }

    #[test]
    fn replacements_update_in_place() {
        let mut center = Center::new(10, false);
        center.notify(note(1, Urgency::Normal));
        let mut update = note(1, Urgency::Normal);
        update.summary = "50%".into();
        assert_eq!(center.notify(update.clone()), [Effect::Popup(1)]);

        center.popup_ended(1, PopupEnd::TimedOut);
        update.summary = "100%".into();
        assert_eq!(center.notify(update), []);
        assert_eq!(center.get(1).unwrap().summary, "100%");
        assert_eq!(history(&center), [1]);
    }

    #[test]
    fn do_not_disturb_lets_only_critical_ones_through() {
        let mut center = Center::new(10, false);
        center.set_dnd(true);
        assert_eq!(center.notify(note(1, Urgency::Normal)), []);
        assert_eq!(
            center.notify(note(2, Urgency::Critical)),
            [Effect::Popup(2)]
        );
        assert_eq!(history(&center), [1]);
    }

    #[test]
    fn the_history_keeps_the_newest() {
        let mut center = Center::new(2, false);
        center.set_dnd(true);
        center.notify(note(1, Urgency::Normal));
        center.notify(note(2, Urgency::Normal));
        assert_eq!(
            center.notify(note(3, Urgency::Normal)),
            [Effect::Closed(1, Reason::Expired)]
        );
        assert_eq!(history(&center), [3, 2]);
        assert_eq!(
            center.clear(),
            [
                Effect::Closed(2, Reason::Dismissed),
                Effect::Closed(3, Reason::Dismissed)
            ]
        );
        assert_eq!(center.history_len(), 0);
    }

    #[test]
    fn apps_close_their_notifications_wherever_they_are() {
        let mut center = Center::new(10, false);
        center.notify(note(1, Urgency::Normal));
        center.notify(note(2, Urgency::Normal));
        center.popup_ended(2, PopupEnd::TimedOut);

        assert_eq!(
            center.close(1, Reason::Closed),
            [Effect::Withdraw(1), Effect::Closed(1, Reason::Closed)]
        );
        assert_eq!(
            center.close(2, Reason::Closed),
            [Effect::Closed(2, Reason::Closed)]
        );
        assert_eq!(center.close(2, Reason::Closed), []);
    }

    #[test]
    fn invoking_an_action_closes_unless_resident() {
        let mut center = Center::new(10, false);
        center.notify(note(1, Urgency::Normal));
        assert_eq!(
            center.invoke(1, "default").unwrap(),
            [
                Effect::Invoked(1, "default".into()),
                Effect::Withdraw(1),
                Effect::Closed(1, Reason::Dismissed)
            ]
        );

        center.notify(Note {
            resident: true,
            ..note(2, Urgency::Normal)
        });
        assert_eq!(
            center.invoke(2, "default").unwrap(),
            [Effect::Invoked(2, "default".into())]
        );
        assert!(center.invoke(2, "reply").is_err());
        assert!(center.invoke(9, "default").is_err());
    }
}

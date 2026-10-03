//! Decides what the island shows.
//!
//! Modules submit activities. Each one carries the attributes the rules below
//! need, so changing the policy for one kind of activity means changing its
//! [`ActivitySpec`], not the arbiter.
//!
//! - One activity is shown at a time.
//! - A new activity **interrupts** the shown one when the shown one is
//!   interruptible and the new one has a higher priority, or the same priority
//!   with [`SamePriority::Stack`]. The interrupted activity is suspended with
//!   its timer paused, and comes back when the new one ends.
//! - Otherwise the new activity **waits** in a queue ordered by priority, then
//!   arrival.
//! - When the shown activity ends, the next one is the most recently
//!   suspended activity or the head of the queue, whichever has the higher
//!   priority. Suspended activities win ties: they were on screen first.
//! - An activity with a `key` **replaces** an existing activity from the same
//!   module with the same key, wherever it is, instead of adding a new one.
//! - Timeouts only run while an activity is on screen and the pointer is not
//!   over it, and not while it is expanded. When the timer starts again, the
//!   activity gets at least [`MIN_VISIBLE`] more.
//! - An activity with [`ActivitySpec::expand_for`] opens on its expanded view
//!   the next time it comes on screen, and collapses on its own once that time
//!   ran out with the pointer away. A click collapses it at once, and a view
//!   the user expanded stays expanded.
//!
//! The arbiter never reads the clock: every call takes `now`. Changes are
//! reported as [`Effect`]s for the daemon to act on.

use std::mem;
use std::time::{Duration, Instant};

use mochi_protocol::{Activity, ActivityId};
use serde_json::Value;

/// When a paused timer starts again, at least this much time is left, so an
/// activity never flashes back for a few milliseconds.
pub const MIN_VISIBLE: Duration = Duration::from_secs(1);

/// Higher values win. The named levels leave room for anything in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priority(pub u8);

impl Priority {
    /// The idle island. Everything else interrupts it.
    pub const IDLE: Self = Self(0);
    pub const LOW: Self = Self(25);
    pub const NORMAL: Self = Self(50);
    pub const HIGH: Self = Self(75);
    pub const URGENT: Self = Self(100);
}

/// What a new activity does when the shown one has the same priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SamePriority {
    /// Wait until the shown activity ends.
    #[default]
    Queue,
    /// Interrupt it. It comes back afterwards.
    Stack,
}

/// Everything a module says about an activity it wants to show.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivitySpec {
    /// Replaces the module's existing activity with the same key.
    pub key: Option<String>,
    /// View shown by default: `modules/<module>/<compact>.qml`.
    pub compact: String,
    /// View shown after a click. Without one, clicks go to the module.
    pub expanded: Option<String>,
    /// Opens on the expanded view for this long, then collapses.
    pub expand_for: Option<Duration>,
    pub payload: Value,
    pub priority: Priority,
    /// Time on screen before the activity ends. `None` never times out.
    pub timeout: Option<Duration>,
    pub interruptible: bool,
    pub same_priority: SamePriority,
}

impl ActivitySpec {
    /// A normal-priority, interruptible activity with no timeout.
    pub fn new(compact: impl Into<String>) -> Self {
        Self {
            key: None,
            compact: compact.into(),
            expanded: None,
            expand_for: None,
            payload: Value::Null,
            priority: Priority::NORMAL,
            timeout: None,
            interruptible: true,
            same_priority: SamePriority::Queue,
        }
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn expanded(mut self, view: impl Into<String>) -> Self {
        self.expanded = Some(view.into());
        self
    }

    /// Opens on the expanded view, which needs one, and collapses after
    /// `duration` on screen. A keyed replacement that sets it again opens it
    /// again.
    pub fn expand_for(mut self, duration: Duration) -> Self {
        self.expand_for = Some(duration);
        self
    }

    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn uninterruptible(mut self) -> Self {
        self.interruptible = false;
        self
    }

    pub fn same_priority(mut self, rule: SamePriority) -> Self {
        self.same_priority = rule;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// The island should show this now. At most one per batch of effects.
    Present(Option<Activity>),
    /// A click on an activity without an expanded view.
    Clicked {
        module: String,
        activity: ActivityId,
    },
    /// The activity is gone and won't come back.
    Ended {
        module: String,
        activity: ActivityId,
        reason: EndReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// Its timeout ran out.
    Expired,
    /// The user closed it.
    Dismissed,
    /// Its module withdrew it.
    Withdrawn,
    /// A newer activity with the same key took its place.
    Replaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArbiterError {
    #[error("activity {0} does not exist")]
    UnknownActivity(ActivityId),
    #[error("activity {0} belongs to another module")]
    NotOwner(ActivityId),
}

#[derive(Debug, Default)]
pub struct Arbiter {
    /// `None` only when nothing else exists either.
    current: Option<Entry>,
    /// Interrupted activities. The last one resumes first.
    suspended: Vec<Entry>,
    /// Waiting activities, highest priority first, then oldest first.
    queue: Vec<Entry>,
    effects: Vec<Effect>,
    /// The shown activity changed since the last `take_effects`.
    dirty: bool,
}

impl Arbiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// What the island shows now.
    pub fn shown(&self) -> Option<Activity> {
        self.current.as_ref().map(Entry::activity)
    }

    /// When the shown activity collapses or its timeout runs out, whichever
    /// comes first, if a timer is running.
    pub fn next_deadline(&self) -> Option<Instant> {
        let entry = self.current.as_ref()?;
        let collapse = entry.collapse.as_ref().and_then(Timer::deadline);
        collapse.into_iter().chain(entry.timer.deadline()).min()
    }

    /// Everything that changed since the last call.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        if mem::take(&mut self.dirty) {
            self.effects.push(Effect::Present(self.shown()));
        }
        mem::take(&mut self.effects)
    }

    pub fn submit(&mut self, id: ActivityId, module: &str, spec: ActivitySpec, now: Instant) {
        if let Some(key) = &spec.key {
            let found = self.locate(|entry| {
                entry.module == module && entry.spec.key.as_deref() == Some(key.as_str())
            });
            if let Some(slot) = found {
                self.replace(slot, id, spec, now);
                return;
            }
        }

        let entry = Entry::new(id, module, spec);

        match self.current.take() {
            None => self.show(entry, now),
            Some(mut current) if entry.interrupts(&current) => {
                current.suspend(now);
                self.suspended.push(current);
                self.show(entry, now);
            }
            Some(current) => {
                self.current = Some(current);
                self.enqueue(entry);
            }
        }
    }

    /// Replaces an activity's payload, wherever it is.
    pub fn update(
        &mut self,
        module: &str,
        id: ActivityId,
        payload: Value,
    ) -> Result<(), ArbiterError> {
        let slot = self.owned(module, id)?;
        self.entry_mut(slot).spec.payload = payload;
        if slot == Slot::Current {
            self.dirty = true;
        }
        Ok(())
    }

    /// Removes an activity, wherever it is.
    pub fn withdraw(
        &mut self,
        module: &str,
        id: ActivityId,
        now: Instant,
    ) -> Result<(), ArbiterError> {
        let slot = self.owned(module, id)?;
        self.remove(slot, EndReason::Withdrawn, now);
        Ok(())
    }

    /// Removes every activity of a module, for example when it stops.
    pub fn withdraw_all(&mut self, module: &str, now: Instant) {
        while let Some(slot) = self.locate(|entry| entry.module == module) {
            self.remove(slot, EndReason::Withdrawn, now);
        }
    }

    /// The pointer entered or left the island. Events for an activity that is
    /// no longer shown are ignored: they arrive during transitions.
    pub fn hover(&mut self, id: ActivityId, hovered: bool, now: Instant) {
        if let Some(current) = self.current_mut(id) {
            current.hovered = hovered;
            current.update_timer(now);
        }
    }

    /// Toggles the expanded view, or reports the click to the module when
    /// there is none.
    pub fn click(&mut self, id: ActivityId, now: Instant) {
        let Some(current) = self.current_mut(id) else {
            return;
        };
        if current.expandable() {
            current.expanded = !current.expanded;
            // The user decides from now on.
            current.collapse = None;
            current.update_timer(now);
            self.dirty = true;
        } else {
            let effect = Effect::Clicked {
                module: current.module.clone(),
                activity: id,
            };
            self.effects.push(effect);
        }
    }

    /// The user closed the shown activity.
    pub fn dismiss(&mut self, id: ActivityId, now: Instant) {
        if self.current_mut(id).is_some() {
            self.end_current(EndReason::Dismissed, now);
        }
    }

    /// Collapses the shown activity or ends it if its time ran out.
    pub fn tick(&mut self, now: Instant) {
        if let Some(current) = &mut self.current
            && current
                .collapse
                .as_ref()
                .and_then(Timer::deadline)
                .is_some_and(|deadline| deadline <= now)
        {
            current.collapse = None;
            current.expanded = false;
            current.update_timer(now);
            self.dirty = true;
        }
        if self.next_deadline().is_some_and(|deadline| deadline <= now) {
            self.end_current(EndReason::Expired, now);
        }
    }

    fn show(&mut self, mut entry: Entry, now: Instant) {
        entry.hovered = false;
        entry.auto_expand();
        entry.update_timer(now);
        entry.seen = true;
        self.current = Some(entry);
        self.dirty = true;
    }

    /// Inserts after every activity with the same or a higher priority, which
    /// keeps arrival order within a priority.
    fn enqueue(&mut self, entry: Entry) {
        let index = self
            .queue
            .partition_point(|queued| queued.spec.priority >= entry.spec.priority);
        self.queue.insert(index, entry);
    }

    fn end_current(&mut self, reason: EndReason, now: Instant) {
        if let Some(entry) = self.current.take() {
            self.ended(entry, reason);
            self.dirty = true;
            self.promote(now);
        }
    }

    /// Shows the next activity after the shown one ended.
    fn promote(&mut self, now: Instant) {
        let resume = match (self.suspended.last(), self.queue.first()) {
            (Some(suspended), Some(queued)) => suspended.spec.priority >= queued.spec.priority,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => return,
        };
        let entry = if resume {
            self.suspended.pop()
        } else {
            Some(self.queue.remove(0))
        };
        if let Some(entry) = entry {
            self.show(entry, now);
        }
    }

    fn replace(&mut self, slot: Slot, id: ActivityId, spec: ActivitySpec, now: Instant) {
        let entry = self.entry_mut(slot);
        let replaced = entry.id;
        let module = entry.module.clone();

        entry.id = id;
        entry.timer = Timer::new(spec.timeout);
        entry.expanded &= spec.expanded.is_some();
        if !entry.expanded {
            entry.collapse = None;
        }
        if spec.expand_for.is_some() {
            entry.pending_expand = spec.expand_for;
        }
        entry.spec = spec;
        if slot == Slot::Current {
            // A fresh timeout, as if it had just appeared.
            entry.auto_expand();
            entry.seen = false;
            entry.update_timer(now);
            entry.seen = true;
            self.dirty = true;
        }

        if let Slot::Queued(index) = slot {
            let entry = self.queue.remove(index);
            self.enqueue(entry);
        }

        self.effects.push(Effect::Ended {
            module,
            activity: replaced,
            reason: EndReason::Replaced,
        });
    }

    fn remove(&mut self, slot: Slot, reason: EndReason, now: Instant) {
        match slot {
            Slot::Current => self.end_current(reason, now),
            Slot::Suspended(index) => {
                let entry = self.suspended.remove(index);
                self.ended(entry, reason);
            }
            Slot::Queued(index) => {
                let entry = self.queue.remove(index);
                self.ended(entry, reason);
            }
        }
    }

    fn ended(&mut self, entry: Entry, reason: EndReason) {
        self.effects.push(Effect::Ended {
            module: entry.module,
            activity: entry.id,
            reason,
        });
    }

    fn current_mut(&mut self, id: ActivityId) -> Option<&mut Entry> {
        self.current.as_mut().filter(|entry| entry.id == id)
    }

    fn owned(&self, module: &str, id: ActivityId) -> Result<Slot, ArbiterError> {
        let slot = self
            .locate(|entry| entry.id == id)
            .ok_or(ArbiterError::UnknownActivity(id))?;
        if self.entry(slot).module != module {
            return Err(ArbiterError::NotOwner(id));
        }
        Ok(slot)
    }

    fn locate(&self, matches: impl Fn(&Entry) -> bool) -> Option<Slot> {
        if self.current.as_ref().is_some_and(&matches) {
            return Some(Slot::Current);
        }
        if let Some(index) = self.suspended.iter().position(&matches) {
            return Some(Slot::Suspended(index));
        }
        self.queue.iter().position(&matches).map(Slot::Queued)
    }

    fn entry(&self, slot: Slot) -> &Entry {
        match slot {
            Slot::Current => self.current.as_ref().expect("located"),
            Slot::Suspended(index) => &self.suspended[index],
            Slot::Queued(index) => &self.queue[index],
        }
    }

    fn entry_mut(&mut self, slot: Slot) -> &mut Entry {
        match slot {
            Slot::Current => self.current.as_mut().expect("located"),
            Slot::Suspended(index) => &mut self.suspended[index],
            Slot::Queued(index) => &mut self.queue[index],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Current,
    Suspended(usize),
    Queued(usize),
}

#[derive(Debug)]
struct Entry {
    id: ActivityId,
    module: String,
    spec: ActivitySpec,
    timer: Timer,
    expanded: bool,
    /// Runs while the activity is expanded on its own, and collapses it when
    /// it runs out.
    collapse: Option<Timer>,
    /// How long to expand the next time it is on screen.
    pending_expand: Option<Duration>,
    hovered: bool,
    /// Has been on screen before. Its timer then gets `MIN_VISIBLE` when it
    /// starts again.
    seen: bool,
}

impl Entry {
    fn new(id: ActivityId, module: &str, spec: ActivitySpec) -> Self {
        Self {
            id,
            module: module.to_owned(),
            timer: Timer::new(spec.timeout),
            pending_expand: spec.expand_for,
            spec,
            expanded: false,
            collapse: None,
            hovered: false,
            seen: false,
        }
    }

    fn interrupts(&self, current: &Entry) -> bool {
        current.spec.interruptible
            && (self.spec.priority > current.spec.priority
                || (self.spec.priority == current.spec.priority
                    && self.spec.same_priority == SamePriority::Stack))
    }

    fn suspend(&mut self, now: Instant) {
        self.timer.pause(now);
        self.expanded = false;
        self.collapse = None;
        self.hovered = false;
    }

    /// Opens the expanded view if the spec asked for it since the activity
    /// was last on screen.
    fn auto_expand(&mut self) {
        let Some(duration) = self.pending_expand.take() else {
            return;
        };
        // An expanded view the user opened stays open.
        if !self.expandable() || (self.expanded && self.collapse.is_none()) {
            return;
        }
        self.expanded = true;
        self.collapse = Some(Timer::new(Some(duration)));
    }

    fn expandable(&self) -> bool {
        self.spec.expanded.is_some()
    }

    fn update_timer(&mut self, now: Instant) {
        let floor = if self.seen {
            MIN_VISIBLE
        } else {
            Duration::ZERO
        };
        if let Some(collapse) = &mut self.collapse {
            if self.hovered {
                collapse.pause(now);
            } else {
                collapse.run(now, floor);
            }
        }
        if self.hovered || self.expanded {
            self.timer.pause(now);
        } else {
            self.timer.run(now, floor);
        }
    }

    fn activity(&self) -> Activity {
        let view = match (&self.spec.expanded, self.expanded) {
            (Some(expanded), true) => expanded.clone(),
            _ => self.spec.compact.clone(),
        };
        Activity {
            id: self.id,
            module: self.module.clone(),
            view,
            payload: self.spec.payload.clone(),
            expanded: self.expanded,
            expandable: self.expandable(),
            key: self.spec.key.clone(),
        }
    }
}

/// A countdown that only runs while started.
#[derive(Debug, Clone)]
struct Timer {
    /// `None` never runs out.
    remaining: Option<Duration>,
    running_since: Option<Instant>,
}

impl Timer {
    fn new(timeout: Option<Duration>) -> Self {
        Self {
            remaining: timeout,
            running_since: None,
        }
    }

    /// Starts the countdown with at least `floor` left.
    fn run(&mut self, now: Instant, floor: Duration) {
        if self.running_since.is_none() {
            self.remaining = self.remaining.map(|remaining| remaining.max(floor));
            self.running_since = Some(now);
        }
    }

    fn pause(&mut self, now: Instant) {
        if let Some(since) = self.running_since.take() {
            let elapsed = now.saturating_duration_since(since);
            self.remaining = self
                .remaining
                .map(|remaining| remaining.saturating_sub(elapsed));
        }
    }

    fn deadline(&self) -> Option<Instant> {
        Some(self.running_since? + self.remaining?)
    }
}

#[cfg(test)]
mod tests;

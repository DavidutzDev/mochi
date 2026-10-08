//! An island per monitor: an [`Arbiter`] for each, so a panel open on one
//! monitor doesn't hold back a notice on another.
//!
//! An activity that names a monitor goes to that monitor's arbiter. One
//! that doesn't, like the idle clock or an overlay, is meant for every
//! monitor: each arbiter gets a copy with the same id, and a monitor that
//! appears later gets one too. When a copy ends on one monitor, by its
//! timeout or a dismissal, the others go too, and its module hears it once.
//!
//! Until the compositor names its monitors, one arbiter under the empty
//! name stands for all of them.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;

use mochi_protocol::{Activity, ActivityId};
use serde_json::Value;

use crate::arbiter::{ActivitySpec, Arbiter, ArbiterError, Effect};

/// The arbiter for every monitor while none is known.
const ANY: &str = "";

/// What changed, for the daemon: [`Effect`]s, with the monitor each new
/// activity on the island is for.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// The island on `output` shows this now; `output` is empty for every
    /// monitor.
    Present {
        output: String,
        activity: Option<Activity>,
    },
    /// Anything else, as the arbiter said it.
    Effect(Effect),
}

#[derive(Debug)]
pub struct Islands {
    arbiters: BTreeMap<String, Arbiter>,
    /// The activities meant for every monitor, as last submitted or
    /// updated, for a monitor that appears.
    everywhere: Vec<(ActivityId, String, ActivitySpec)>,
    /// The activities meant for one monitor: their module, key and monitor.
    placed: HashMap<ActivityId, (String, Option<String>, String)>,
    /// Changes from a monitor that went away, for the next `take_effects`.
    pending: Vec<Change>,
    outside_expanded_only: bool,
    exclusive: Option<String>,
}

impl Default for Islands {
    fn default() -> Self {
        Self {
            arbiters: BTreeMap::from([(ANY.to_owned(), Arbiter::new())]),
            everywhere: Vec::new(),
            placed: HashMap::new(),
            pending: Vec::new(),
            outside_expanded_only: false,
            exclusive: None,
        }
    }
}

impl Islands {
    pub fn new() -> Self {
        Self::default()
    }

    /// The monitors there are now. A new one gets every activity meant for
    /// all of them; the activities of one that went away end.
    pub fn set_outputs(&mut self, outputs: &[String], now: Instant) {
        let wanted: Vec<String> = if outputs.is_empty() {
            vec![ANY.to_owned()]
        } else {
            outputs.to_vec()
        };
        let gone: Vec<String> = self
            .arbiters
            .keys()
            .filter(|name| !wanted.contains(name))
            .cloned()
            .collect();
        for name in &wanted {
            self.ensure(name, now);
        }
        for name in gone {
            // Only what was meant for it ends; copies live on elsewhere.
            if let Some(arbiter) = self.arbiters.get_mut(&name) {
                for (id, module) in arbiter.activities() {
                    if !self.everywhere.iter().any(|(other, ..)| *other == id) {
                        let _ = arbiter.withdraw(&module, id, now);
                        self.placed.remove(&id);
                    }
                }
            }
            if let Some(mut arbiter) = self.arbiters.remove(&name) {
                // Keep its endings; its island is gone, so it shows nothing.
                self.pending.extend(
                    arbiter
                        .take_effects()
                        .into_iter()
                        .filter(|effect| !matches!(effect, Effect::Present(_)))
                        .map(Change::Effect),
                );
                self.pending.push(Change::Present {
                    output: name,
                    activity: None,
                });
            }
        }
    }

    /// The arbiter for `output`, made with every activity meant for all.
    fn ensure(&mut self, output: &str, now: Instant) -> &mut Arbiter {
        if !self.arbiters.contains_key(output) {
            let mut arbiter = Arbiter::new();
            arbiter.set_outside_expanded_only(self.outside_expanded_only);
            arbiter.set_exclusive(self.exclusive.clone(), now);
            for (id, module, spec) in &self.everywhere {
                arbiter.submit(*id, module, spec.clone(), now);
            }
            self.arbiters.insert(output.to_owned(), arbiter);
        }
        self.arbiters.get_mut(output).expect("made above")
    }

    pub fn submit(&mut self, id: ActivityId, module: &str, spec: ActivitySpec, now: Instant) {
        // A keyed activity replaces the module's one with that key on every
        // monitor, like a volume notice following the focus. Each arbiter
        // replaces its own; the others' go here.
        if let Some(key) = spec.key.clone() {
            let target = spec.output.as_deref().map(|output| self.target(output));
            let stale: Vec<ActivityId> = self
                .placed
                .iter()
                .filter(|(_, (owner, other, output))| {
                    owner == module
                        && other.as_deref() == Some(key.as_str())
                        && Some(output.as_str()) != target.as_deref()
                })
                .map(|(other, _)| *other)
                .collect();
            let stale_everywhere: Vec<ActivityId> = if spec.output.is_some() {
                self.everywhere
                    .iter()
                    .filter(|(_, owner, other)| {
                        owner == module && other.key.as_deref() == Some(key.as_str())
                    })
                    .map(|(other, ..)| *other)
                    .collect()
            } else {
                Vec::new()
            };
            for other in stale.into_iter().chain(stale_everywhere) {
                let _ = self.withdraw(module, other, now);
            }
            if let Some(target) = &target {
                // Replaced in place by the arbiter: no longer tracked.
                self.placed.retain(|_, (owner, other, output)| {
                    !(owner == module && other.as_deref() == Some(key.as_str()) && output == target)
                });
            }
        }
        match spec.output.clone() {
            Some(output) => {
                let target = self.target(&output);
                self.placed
                    .insert(id, (module.to_owned(), spec.key.clone(), target.clone()));
                self.ensure(&target, now).submit(id, module, spec, now);
            }
            None => {
                if let Some(key) = &spec.key {
                    self.everywhere.retain(|(_, owner, other)| {
                        !(owner == module && other.key.as_deref() == Some(key.as_str()))
                    });
                }
                self.everywhere.push((id, module.to_owned(), spec.clone()));
                for arbiter in self.arbiters.values_mut() {
                    arbiter.submit(id, module, spec.clone(), now);
                }
            }
        }
    }

    /// The arbiter an activity for `output` goes to: the stand-in until
    /// monitors are known.
    fn target(&self, output: &str) -> String {
        if self.arbiters.contains_key(ANY) {
            ANY.to_owned()
        } else {
            output.to_owned()
        }
    }

    pub fn update(
        &mut self,
        module: &str,
        id: ActivityId,
        payload: Value,
    ) -> Result<(), ArbiterError> {
        if let Some((_, _, spec)) = self.everywhere.iter_mut().find(|(other, ..)| *other == id) {
            spec.payload = payload.clone();
        }
        let mut found = false;
        for arbiter in self
            .arbiters
            .values_mut()
            .filter(|arbiter| arbiter.contains(id))
        {
            arbiter.update(module, id, payload.clone())?;
            found = true;
        }
        if found {
            Ok(())
        } else {
            Err(ArbiterError::UnknownActivity(id))
        }
    }

    pub fn withdraw(
        &mut self,
        module: &str,
        id: ActivityId,
        now: Instant,
    ) -> Result<(), ArbiterError> {
        let mut found = false;
        // Every copy has the same owner: a wrong one fails at the first.
        for arbiter in self
            .arbiters
            .values_mut()
            .filter(|arbiter| arbiter.contains(id))
        {
            arbiter.withdraw(module, id, now)?;
            found = true;
        }
        self.everywhere.retain(|(other, ..)| *other != id);
        self.placed.remove(&id);
        if found {
            Ok(())
        } else {
            Err(ArbiterError::UnknownActivity(id))
        }
    }

    pub fn withdraw_all(&mut self, module: &str, now: Instant) {
        self.everywhere.retain(|(_, owner, _)| owner != module);
        self.placed.retain(|_, (owner, ..)| owner != module);
        for arbiter in self.arbiters.values_mut() {
            arbiter.withdraw_all(module, now);
        }
    }

    /// The arbiters an event from the island on `output` is for: that
    /// monitor's, when it holds the activity, or else every one that does.
    fn routed(&mut self, output: Option<&str>, id: ActivityId) -> Vec<&mut Arbiter> {
        let here = output
            .filter(|output| {
                self.arbiters
                    .get(*output)
                    .is_some_and(|arbiter| arbiter.contains(id))
            })
            .map(str::to_owned);
        self.arbiters
            .iter_mut()
            .filter(|(name, arbiter)| match &here {
                Some(here) => *name == here,
                None => arbiter.contains(id),
            })
            .map(|(_, arbiter)| arbiter)
            .collect()
    }

    pub fn hover(&mut self, output: Option<&str>, id: ActivityId, hovered: bool, now: Instant) {
        for arbiter in self.routed(output, id) {
            arbiter.hover(id, hovered, now);
        }
    }

    pub fn click(&mut self, output: Option<&str>, id: ActivityId, now: Instant) {
        for arbiter in self.routed(output, id) {
            arbiter.click(id, now);
        }
    }

    pub fn outside(&mut self, output: Option<&str>, id: ActivityId, now: Instant) {
        for arbiter in self.routed(output, id) {
            arbiter.outside(id, now);
        }
    }

    /// Closes an activity on every monitor that has it.
    pub fn dismiss(&mut self, id: ActivityId, now: Instant) {
        for arbiter in self.arbiters.values_mut() {
            arbiter.dismiss(id, now);
        }
    }

    pub fn tick(&mut self, now: Instant) {
        for arbiter in self.arbiters.values_mut() {
            arbiter.tick(now);
        }
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.arbiters
            .values()
            .filter_map(Arbiter::next_deadline)
            .min()
    }

    pub fn set_exclusive(&mut self, module: Option<String>, now: Instant) {
        self.exclusive.clone_from(&module);
        for arbiter in self.arbiters.values_mut() {
            arbiter.set_exclusive(module.clone(), now);
        }
    }

    pub fn exclusive(&self) -> Option<&str> {
        self.exclusive.as_deref()
    }

    pub fn set_outside_expanded_only(&mut self, only: bool) {
        self.outside_expanded_only = only;
        for arbiter in self.arbiters.values_mut() {
            arbiter.set_outside_expanded_only(only);
        }
    }

    /// What each monitor's island shows, by monitor; the empty name stands
    /// for all of them.
    pub fn shown(&self) -> BTreeMap<String, Option<Activity>> {
        self.arbiters
            .iter()
            .map(|(name, arbiter)| (name.clone(), arbiter.shown()))
            .collect()
    }

    /// What the island on `output` shows, or on any monitor without one.
    pub fn shown_on(&self, output: Option<&str>) -> Option<Activity> {
        output
            .and_then(|output| self.arbiters.get(output))
            .and_then(Arbiter::shown)
            .or_else(|| self.arbiters.values().find_map(Arbiter::shown))
    }

    /// Everything that changed since the last call. A copy meant for every
    /// monitor that ended on one ends on the others, said once.
    pub fn take_effects(&mut self, now: Instant) -> Vec<Change> {
        let mut changes: Vec<Change> = std::mem::take(&mut self.pending);
        let mut ended: HashSet<ActivityId> = HashSet::new();
        for change in &changes {
            if let Change::Effect(Effect::Ended { activity, .. }) = change {
                ended.insert(*activity);
            }
        }
        loop {
            for (name, arbiter) in &mut self.arbiters {
                for effect in arbiter.take_effects() {
                    match effect {
                        Effect::Present(activity) => changes.push(Change::Present {
                            output: name.clone(),
                            activity,
                        }),
                        Effect::Ended { activity, .. } => {
                            self.placed.remove(&activity);
                            if ended.insert(activity) {
                                changes.push(Change::Effect(effect));
                            }
                        }
                        other => changes.push(Change::Effect(other)),
                    }
                }
            }
            // Copies of what ended on one monitor still on others.
            let leftover: Vec<(ActivityId, String)> = self
                .everywhere
                .iter()
                .filter(|(id, ..)| ended.contains(id))
                .map(|(id, module, _)| (*id, module.clone()))
                .collect();
            if leftover.is_empty() {
                break;
            }
            self.everywhere.retain(|(id, ..)| !ended.contains(id));
            for (id, module) in leftover {
                for arbiter in self.arbiters.values_mut() {
                    if arbiter.contains(id) {
                        let _ = arbiter.withdraw(&module, id, now);
                    }
                }
            }
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use mochi_protocol::spec::Priority;

    use super::*;
    use crate::EndReason;

    fn outputs(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// What each named monitor's island shows after `changes`.
    fn presented(changes: &[Change]) -> BTreeMap<String, Option<String>> {
        changes
            .iter()
            .filter_map(|change| match change {
                Change::Present { output, activity } if !output.is_empty() => Some((
                    output.clone(),
                    activity.as_ref().map(|activity| activity.view.clone()),
                )),
                _ => None,
            })
            .collect()
    }

    fn ended(changes: &[Change]) -> Vec<(ActivityId, EndReason)> {
        changes
            .iter()
            .filter_map(|change| match change {
                Change::Effect(Effect::Ended {
                    activity, reason, ..
                }) => Some((*activity, *reason)),
                _ => None,
            })
            .collect()
    }

    fn idle() -> ActivitySpec {
        ActivitySpec::new("Clock").priority(Priority::IDLE)
    }

    #[test]
    fn a_panel_on_one_monitor_leaves_the_other_free() {
        let now = Instant::now();
        let mut islands = Islands::new();
        islands.set_outputs(&outputs(&["DP-1", "HDMI-A-1"]), now);
        islands.submit(ActivityId(1), "idle", idle(), now);
        let changes = islands.take_effects(now);
        // The stand-in for every monitor shows nothing now.
        assert!(changes.contains(&Change::Present {
            output: String::new(),
            activity: None
        }));
        assert_eq!(
            presented(&changes),
            BTreeMap::from([
                ("DP-1".into(), Some("Clock".into())),
                ("HDMI-A-1".into(), Some("Clock".into()))
            ])
        );

        let mut panel = ActivitySpec::new("Hub")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal();
        panel.output = Some("DP-1".into());
        islands.submit(ActivityId(2), "hub", panel, now);
        let mut notice = ActivitySpec::new("Volume").priority(Priority::HIGH);
        notice.output = Some("HDMI-A-1".into());
        islands.submit(ActivityId(3), "osd", notice, now);
        let changes = islands.take_effects(now);
        // The notice didn't wait behind the hub.
        assert_eq!(
            presented(&changes),
            BTreeMap::from([
                ("DP-1".into(), Some("Hub".into())),
                ("HDMI-A-1".into(), Some("Volume".into()))
            ])
        );
        assert_eq!(islands.shown_on(Some("HDMI-A-1")).unwrap().view, "Volume");
    }

    #[test]
    fn what_shows_everywhere_ends_once() {
        let now = Instant::now();
        let mut islands = Islands::new();
        islands.set_outputs(&outputs(&["DP-1", "HDMI-A-1"]), now);
        let notice = ActivitySpec::new("Notice").priority(Priority::HIGH);
        islands.submit(ActivityId(5), "battery", notice, now);
        islands.take_effects(now);
        islands.dismiss(ActivityId(5), now);
        let changes = islands.take_effects(now);
        assert_eq!(ended(&changes), [(ActivityId(5), EndReason::Dismissed)]);
        assert!(islands.shown().values().all(Option::is_none));
    }

    #[test]
    fn a_click_expands_on_its_monitor_only() {
        let now = Instant::now();
        let mut islands = Islands::new();
        islands.set_outputs(&outputs(&["DP-1", "HDMI-A-1"]), now);
        let media = ActivitySpec::new("Compact").expanded("Expanded");
        islands.submit(ActivityId(7), "media", media, now);
        islands.take_effects(now);
        islands.click(Some("DP-1"), ActivityId(7), now);
        let shown = islands.shown();
        assert!(shown["DP-1"].as_ref().unwrap().expanded);
        assert!(!shown["HDMI-A-1"].as_ref().unwrap().expanded);
    }

    #[test]
    fn monitors_come_and_go() {
        let now = Instant::now();
        let mut islands = Islands::new();
        // Before the compositor says, one island for all.
        islands.submit(ActivityId(1), "idle", idle(), now);
        let mut panel = ActivitySpec::new("Launcher").modal();
        panel.output = Some("DP-1".into());
        islands.submit(ActivityId(2), "launcher", panel, now);
        assert_eq!(islands.shown_on(None).unwrap().view, "Launcher");

        islands.set_outputs(&outputs(&["DP-1"]), now);
        islands.take_effects(now);
        // The clock came along to the new monitor's island.
        assert_eq!(islands.shown_on(Some("DP-1")).unwrap().view, "Clock");

        islands.set_outputs(&outputs(&["DP-1", "HDMI-A-1"]), now);
        assert_eq!(islands.shown_on(Some("HDMI-A-1")).unwrap().view, "Clock");
        let mut notice = ActivitySpec::new("Notice").priority(Priority::HIGH);
        notice.output = Some("HDMI-A-1".into());
        islands.submit(ActivityId(3), "osd", notice, now);
        islands.take_effects(now);
        islands.set_outputs(&outputs(&["DP-1"]), now);
        let changes = islands.take_effects(now);
        // Its notice ended with it; the clock lives on.
        assert_eq!(ended(&changes), [(ActivityId(3), EndReason::Withdrawn)]);
        assert_eq!(islands.shown_on(Some("DP-1")).unwrap().view, "Clock");
    }
}

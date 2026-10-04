//! Keeps the bubbles and decides where each one goes.
//!
//! A module says which area a bubble wants, which group it joins and its
//! order. The user's `[bubbles.<module>]` settings override any of those for
//! all of a module's bubbles. Within an area, bubbles sort from left to
//! right by `order`, then by priority, then by age. A group takes
//! the place of its first member, and its other members follow it, so the
//! whole group is drawn as one pill. Past `max_per_area`, an area leaves
//! out its lowest priorities and counts them instead.
//!
//! Like the arbiter, this never talks to anything. The daemon sends the
//! [`Bubbles::snapshot`] to the UI whenever [`Bubbles::take_changed`] says
//! so.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use mochi_protocol::{Area, Bubble, BubbleId, Overflow};
use serde::Deserialize;
use serde_json::Value;

use crate::Priority;

/// Everything a module says about a bubble it wants to show.
#[derive(Debug, Clone, PartialEq)]
pub struct BubbleSpec {
    /// Replaces the module's existing bubble with the same key, keeping its
    /// place.
    pub key: Option<String>,
    /// `modules/<module>/<view>.qml`: a small view that fits a round bubble,
    /// an icon or a cover.
    pub view: String,
    /// A wider view with text, shown in a pill when the user asks for it
    /// with `wide = true`.
    pub wide: Option<String>,
    pub payload: Value,
    pub area: Area,
    /// Bubbles with the same group in the same area share one pill.
    pub group: Option<String>,
    /// Lower goes further left.
    pub order: i32,
    /// Breaks ties in `order`, and decides who is left out when an area is
    /// full.
    pub priority: Priority,
}

impl BubbleSpec {
    /// A normal-priority bubble on its own, right of the center.
    pub fn new(view: impl Into<String>) -> Self {
        Self {
            key: None,
            view: view.into(),
            wide: None,
            payload: Value::Null,
            area: Area::CenterRight,
            group: None,
            order: 0,
            priority: Priority::NORMAL,
        }
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn wide(mut self, view: impl Into<String>) -> Self {
        self.wide = Some(view.into());
        self
    }

    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn area(mut self, area: Area) -> Self {
        self.area = area;
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }
}

/// The user's placement for all of one module's bubbles:
/// `[bubbles.<module>]` in `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Placement {
    pub area: Option<Area>,
    /// `""` gives each bubble a pill of its own.
    pub group: Option<String>,
    pub order: Option<i32>,
    /// Shows the module's wide views, for modules that have them.
    pub wide: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BubbleError {
    #[error("bubble {0} does not exist")]
    Unknown(BubbleId),
    #[error("bubble {0} belongs to another module")]
    NotOwner(BubbleId),
}

#[derive(Debug, Default)]
pub struct Bubbles {
    entries: Vec<Entry>,
    placements: BTreeMap<String, Placement>,
    /// `None` shows every bubble.
    max_per_area: Option<usize>,
    /// Counts arrivals, so older bubbles sort first on ties.
    arrivals: u64,
    changed: bool,
}

#[derive(Debug)]
struct Entry {
    id: BubbleId,
    module: String,
    spec: BubbleSpec,
    arrival: u64,
}

/// Where an entry ends up once the user's placement applies.
#[derive(Debug)]
struct Placed<'a> {
    entry: &'a Entry,
    area: Area,
    group: Option<&'a str>,
    order: i32,
    /// The wide view, when the user asked for it and the module has one.
    wide: Option<&'a str>,
}

impl Bubbles {
    pub fn new(placements: BTreeMap<String, Placement>, max_per_area: Option<usize>) -> Self {
        Self {
            placements,
            max_per_area,
            ..Self::default()
        }
    }

    /// Replaces the user's placements and the per-area maximum, for a
    /// reloaded `config.toml`.
    pub fn configure(
        &mut self,
        placements: BTreeMap<String, Placement>,
        max_per_area: Option<usize>,
    ) {
        if self.placements != placements || self.max_per_area != max_per_area {
            self.placements = placements;
            self.max_per_area = max_per_area;
            self.changed = true;
        }
    }

    pub fn show(&mut self, id: BubbleId, module: &str, spec: BubbleSpec) {
        self.changed = true;
        if let Some(key) = &spec.key
            && let Some(entry) = self.entries.iter_mut().find(|entry| {
                entry.module == module && entry.spec.key.as_deref() == Some(key.as_str())
            })
        {
            entry.id = id;
            entry.spec = spec;
            return;
        }
        self.arrivals += 1;
        self.entries.push(Entry {
            id,
            module: module.to_owned(),
            spec,
            arrival: self.arrivals,
        });
    }

    pub fn update(
        &mut self,
        module: &str,
        id: BubbleId,
        payload: Value,
    ) -> Result<(), BubbleError> {
        let index = self.owned(module, id)?;
        self.entries[index].spec.payload = payload;
        self.changed = true;
        Ok(())
    }

    pub fn hide(&mut self, module: &str, id: BubbleId) -> Result<(), BubbleError> {
        let index = self.owned(module, id)?;
        self.entries.remove(index);
        self.changed = true;
        Ok(())
    }

    /// Removes every bubble of a module, for example when it stops.
    pub fn hide_all(&mut self, module: &str) {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.module != module);
        self.changed |= self.entries.len() != before;
    }

    /// The module a bubble belongs to, to route its clicks.
    pub fn owner(&self, id: BubbleId) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.module.as_str())
    }

    /// Whether anything changed since the last call.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Every shown bubble in drawing order, and what each full area left
    /// out.
    pub fn snapshot(&self) -> (Vec<Bubble>, Vec<Overflow>) {
        let mut placed: Vec<Placed<'_>> =
            self.entries.iter().map(|entry| self.place(entry)).collect();
        placed.sort_by_key(|placed| {
            (
                placed.area,
                placed.order,
                Reverse(placed.entry.spec.priority),
                placed.entry.arrival,
            )
        });

        let mut bubbles = Vec::new();
        let mut overflow = Vec::new();
        for area in Area::ALL {
            let mut in_area: Vec<&Placed<'_>> =
                placed.iter().filter(|placed| placed.area == area).collect();
            if let Some(max) = self.max_per_area
                && in_area.len() > max
            {
                let hidden = u32::try_from(in_area.len() - max).unwrap_or(u32::MAX);
                overflow.push(Overflow { area, hidden });
                // The lowest priorities make room, wherever they sort.
                let mut kept = in_area.clone();
                kept.sort_by_key(|placed| {
                    (Reverse(placed.entry.spec.priority), placed.entry.arrival)
                });
                kept.truncate(max);
                in_area.retain(|placed| kept.iter().any(|kept| std::ptr::eq(*kept, *placed)));
            }
            bubbles.extend(gathered(&in_area).into_iter().map(|placed| Bubble {
                id: placed.entry.id,
                module: placed.entry.module.clone(),
                key: placed.entry.spec.key.clone(),
                view: placed.wide.unwrap_or(&placed.entry.spec.view).to_owned(),
                wide: placed.wide.is_some(),
                payload: placed.entry.spec.payload.clone(),
                area,
                group: placed.group.map(str::to_owned),
            }));
        }
        (bubbles, overflow)
    }

    fn place<'a>(&'a self, entry: &'a Entry) -> Placed<'a> {
        let placement = self.placements.get(&entry.module);
        let group = match placement.and_then(|placement| placement.group.as_deref()) {
            Some("") => None,
            Some(group) => Some(group),
            None => entry.spec.group.as_deref(),
        };
        Placed {
            entry,
            area: placement
                .and_then(|placement| placement.area)
                .unwrap_or(entry.spec.area),
            group,
            order: placement
                .and_then(|placement| placement.order)
                .unwrap_or(entry.spec.order),
            wide: entry
                .spec
                .wide
                .as_deref()
                .filter(|_| placement.and_then(|placement| placement.wide) == Some(true)),
        }
    }

    fn owned(&self, module: &str, id: BubbleId) -> Result<usize, BubbleError> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.id == id)
            .ok_or(BubbleError::Unknown(id))?;
        if self.entries[index].module != module {
            return Err(BubbleError::NotOwner(id));
        }
        Ok(index)
    }
}

/// Moves each group's members up behind its first member, keeping the order
/// otherwise.
fn gathered<'a, 'b>(sorted: &[&'b Placed<'a>]) -> Vec<&'b Placed<'a>> {
    let mut result: Vec<&Placed<'_>> = Vec::with_capacity(sorted.len());
    for (index, placed) in sorted.iter().enumerate() {
        let Some(group) = placed.group else {
            result.push(placed);
            continue;
        };
        let first = sorted[..index]
            .iter()
            .all(|earlier| earlier.group != Some(group));
        if first {
            result.extend(
                sorted[index..]
                    .iter()
                    .filter(|later| later.group == Some(group)),
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ids(bubbles: &Bubbles) -> Vec<u64> {
        bubbles
            .snapshot()
            .0
            .iter()
            .map(|bubble| bubble.id.0)
            .collect()
    }

    fn spec(area: Area) -> BubbleSpec {
        BubbleSpec::new("Bubble").area(area)
    }

    #[test]
    fn areas_go_left_to_right_and_sort_inside() {
        let mut bubbles = Bubbles::default();
        bubbles.show(BubbleId(1), "a", spec(Area::Right));
        bubbles.show(BubbleId(2), "a", spec(Area::Left).order(5));
        bubbles.show(BubbleId(3), "b", spec(Area::Left));
        bubbles.show(BubbleId(4), "b", spec(Area::Left).priority(Priority::HIGH));
        bubbles.show(BubbleId(5), "c", spec(Area::Left));
        // Order first, then priority, then age.
        assert_eq!(ids(&bubbles), [4, 3, 5, 2, 1]);
    }

    #[test]
    fn groups_gather_behind_their_first_member() {
        let mut bubbles = Bubbles::default();
        bubbles.show(BubbleId(1), "bt", spec(Area::Right).group("status"));
        bubbles.show(BubbleId(2), "clock", spec(Area::Right).order(1));
        bubbles.show(
            BubbleId(3),
            "volume",
            spec(Area::Right).order(2).group("status"),
        );
        assert_eq!(ids(&bubbles), [1, 3, 2]);

        let (snapshot, _) = bubbles.snapshot();
        assert_eq!(snapshot[1].group.as_deref(), Some("status"));
        assert_eq!(snapshot[2].group, None);
    }

    #[test]
    fn the_user_moves_regroups_and_reorders_a_module() {
        let placements = BTreeMap::from([
            (
                "media".to_owned(),
                Placement {
                    area: Some(Area::Left),
                    group: Some(String::new()),
                    order: Some(-1),
                    wide: None,
                },
            ),
            (
                "bt".to_owned(),
                Placement {
                    group: Some("status".into()),
                    ..Placement::default()
                },
            ),
        ]);
        let mut bubbles = Bubbles::new(placements, None);
        bubbles.show(BubbleId(1), "other", spec(Area::Left));
        bubbles.show(BubbleId(2), "media", spec(Area::CenterLeft).group("music"));
        bubbles.show(BubbleId(3), "bt", spec(Area::Left));

        let (snapshot, _) = bubbles.snapshot();
        let placed: Vec<_> = snapshot
            .iter()
            .map(|bubble| (bubble.id.0, bubble.area, bubble.group.as_deref()))
            .collect();
        assert_eq!(
            placed,
            [
                (2, Area::Left, None),
                (1, Area::Left, None),
                (3, Area::Left, Some("status")),
            ]
        );
    }

    #[test]
    fn bubbles_are_small_unless_the_user_asks_for_wide_ones() {
        let wide = Placement {
            wide: Some(true),
            ..Placement::default()
        };
        let placements =
            BTreeMap::from([("media".to_owned(), wide.clone()), ("bt".to_owned(), wide)]);
        let mut bubbles = Bubbles::new(placements, None);
        bubbles.show(BubbleId(1), "media", spec(Area::Left).wide("Wide"));
        bubbles.show(BubbleId(2), "bt", spec(Area::Left));
        bubbles.show(BubbleId(3), "other", spec(Area::Left).wide("Wide"));

        let (snapshot, _) = bubbles.snapshot();
        let views: Vec<_> = snapshot
            .iter()
            .map(|bubble| (bubble.view.as_str(), bubble.wide))
            .collect();
        // bt has no wide view, and nobody asked for other's.
        assert_eq!(
            views,
            [("Wide", true), ("Bubble", false), ("Bubble", false)]
        );
    }

    #[test]
    fn keyed_bubbles_replace_in_place() {
        let mut bubbles = Bubbles::default();
        bubbles.show(BubbleId(1), "media", spec(Area::Left).key("now"));
        bubbles.show(BubbleId(2), "other", spec(Area::Left));
        bubbles.show(
            BubbleId(3),
            "media",
            spec(Area::Left)
                .key("now")
                .payload(json!({ "title": "Next" })),
        );
        assert_eq!(ids(&bubbles), [3, 2]);
        assert_eq!(bubbles.snapshot().0[0].payload["title"], "Next");
    }

    #[test]
    fn full_areas_leave_out_the_lowest_priorities() {
        let mut bubbles = Bubbles::new(BTreeMap::new(), Some(2));
        bubbles.show(BubbleId(1), "a", spec(Area::Right).priority(Priority::LOW));
        bubbles.show(BubbleId(2), "a", spec(Area::Right).order(9));
        bubbles.show(BubbleId(3), "a", spec(Area::Right));
        bubbles.show(BubbleId(4), "a", spec(Area::Left).priority(Priority::LOW));

        let (snapshot, overflow) = bubbles.snapshot();
        let shown: Vec<u64> = snapshot.iter().map(|bubble| bubble.id.0).collect();
        // The low one goes even though it sorts first; the rest keep their
        // order.
        assert_eq!(shown, [4, 3, 2]);
        assert_eq!(
            overflow,
            [Overflow {
                area: Area::Right,
                hidden: 1
            }]
        );
    }

    #[test]
    fn modules_only_touch_their_own_bubbles() {
        let mut bubbles = Bubbles::default();
        bubbles.show(BubbleId(1), "a", spec(Area::Left));
        assert!(bubbles.take_changed());
        assert!(!bubbles.take_changed());

        assert_eq!(
            bubbles.update("b", BubbleId(1), json!(1)),
            Err(BubbleError::NotOwner(BubbleId(1)))
        );
        assert_eq!(
            bubbles.hide("a", BubbleId(9)),
            Err(BubbleError::Unknown(BubbleId(9)))
        );
        assert_eq!(bubbles.owner(BubbleId(1)), Some("a"));

        bubbles.update("a", BubbleId(1), json!(1)).unwrap();
        assert!(bubbles.take_changed());
        bubbles.hide_all("a");
        assert!(bubbles.take_changed());
        assert_eq!(ids(&bubbles), Vec::<u64>::new());
        bubbles.hide_all("a");
        assert!(!bubbles.take_changed());
    }
}

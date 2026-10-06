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

pub use mochi_protocol::spec::BubbleSpec;

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
    /// `false` hides the module's bubbles. The module keeps running and
    /// still shows them as far as it knows.
    pub show: Option<bool>,
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
    /// Counts news, so the latest has the highest.
    news: u64,
    /// Areas stack their bubbles, when set.
    stack: Option<mochi_protocol::Stacking>,
    changed: bool,
}

#[derive(Debug)]
struct Entry {
    id: BubbleId,
    module: String,
    spec: BubbleSpec,
    arrival: u64,
    /// The bubble's latest news, from `Bubbles::news`.
    news: u64,
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

    /// Stacks each area's bubbles, or lays them side by side with `None`.
    pub fn set_stack(&mut self, stack: Option<mochi_protocol::Stacking>) {
        if self.stack != stack {
            self.stack = stack;
            self.changed = true;
        }
    }

    pub fn stack(&self) -> Option<mochi_protocol::Stacking> {
        self.stack
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
            if spec.news {
                self.news += 1;
                entry.news = self.news;
            }
            entry.id = id;
            entry.spec = spec;
            return;
        }
        self.arrivals += 1;
        self.news += 1;
        self.entries.push(Entry {
            id,
            module: module.to_owned(),
            spec,
            arrival: self.arrivals,
            news: self.news,
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
        let mut placed: Vec<Placed<'_>> = self
            .entries
            .iter()
            .filter(|entry| {
                self.placements
                    .get(&entry.module)
                    .and_then(|placement| placement.show)
                    != Some(false)
            })
            .map(|entry| self.place(entry))
            .collect();
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
            // A stack holds them all.
            if let Some(max) = self.max_per_area.filter(|_| self.stack.is_none())
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
                priority: placed.entry.spec.priority.0,
                news: placed.entry.news,
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
    use crate::Priority;

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

    fn news(bubbles: &Bubbles) -> Vec<(u64, u64)> {
        bubbles
            .snapshot()
            .0
            .iter()
            .map(|bubble| (bubble.id.0, bubble.news))
            .collect()
    }

    #[test]
    fn news_counts_arrivals_and_flagged_showings_only() {
        let mut bubbles = Bubbles::default();
        bubbles.show(BubbleId(1), "a", spec(Area::Right).key("x"));
        bubbles.show(BubbleId(2), "b", spec(Area::Right).key("y"));
        assert_eq!(news(&bubbles), [(1, 1), (2, 2)]);
        // Showing again without news keeps it; an update too.
        bubbles.show(BubbleId(3), "a", spec(Area::Right).key("x"));
        bubbles.update("a", BubbleId(3), json!({ "n": 1 })).unwrap();
        assert_eq!(news(&bubbles), [(3, 1), (2, 2)]);
        bubbles.show(BubbleId(4), "a", spec(Area::Right).key("x").news());
        assert_eq!(news(&bubbles), [(4, 3), (2, 2)]);
    }

    #[test]
    fn a_stack_holds_every_bubble() {
        let mut bubbles = Bubbles::new(BTreeMap::new(), Some(1));
        bubbles.show(BubbleId(1), "a", spec(Area::Right));
        bubbles.show(BubbleId(2), "b", spec(Area::Right).priority(Priority::HIGH));
        assert_eq!(bubbles.snapshot().1.len(), 1);
        bubbles.set_stack(Some(mochi_protocol::Stacking { news_ms: 4000 }));
        let (shown, overflow) = bubbles.snapshot();
        assert_eq!(shown.len(), 2);
        assert!(overflow.is_empty());
        assert_eq!(
            shown
                .iter()
                .find(|bubble| bubble.id.0 == 2)
                .unwrap()
                .priority,
            Priority::HIGH.0
        );
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
                    ..Placement::default()
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
    fn the_user_hides_a_modules_bubbles() {
        let hidden = Placement {
            show: Some(false),
            ..Placement::default()
        };
        let mut bubbles = Bubbles::new(BTreeMap::from([("media".to_owned(), hidden)]), Some(1));
        bubbles.show(
            BubbleId(1),
            "media",
            spec(Area::Left).priority(Priority::HIGH),
        );
        bubbles.show(BubbleId(2), "bt", spec(Area::Left));

        // Hidden ones take no room either.
        let (snapshot, overflow) = bubbles.snapshot();
        assert_eq!(ids(&bubbles), [2]);
        assert_eq!(snapshot.len(), 1);
        assert!(overflow.is_empty());
        // The module can still change and hide them.
        assert_eq!(bubbles.update("media", BubbleId(1), json!({})), Ok(()));
        assert_eq!(bubbles.hide("media", BubbleId(1)), Ok(()));
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

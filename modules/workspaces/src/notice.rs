//! Turns compositor snapshots into notices worth showing.
//!
//! Workspaces are compared by output and name, not by id: compositors may
//! reuse ids once a workspace is gone. The first snapshot, and the first one
//! after the compositor becomes available, only set the baseline.

use std::collections::BTreeMap;

use mochi_core::compositor::{Backend, State, Workspace};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The active workspace on the output changed.
    Switch,
    /// Focus moved to the output without its workspace changing.
    Focus,
    /// A workspace on the output started asking for attention.
    Urgent,
    /// Workspaces were created or removed on the output.
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub output: String,
    pub reason: Reason,
    /// The workspace that became urgent.
    pub urgent: Option<String>,
}

#[derive(Debug, Default)]
pub struct Tracker {
    previous: Option<State>,
}

impl Tracker {
    /// Compares with the previous snapshot. When several things changed at
    /// once, a switch wins over a focus change, then an urgent workspace,
    /// then workspaces created or removed. Among switches, the focused
    /// output's wins.
    pub fn apply(&mut self, next: &State) -> Option<Notice> {
        let previous = self.previous.replace(next.clone())?;
        if previous.backend == Backend::Unsupported || next.backend == Backend::Unsupported {
            return None;
        }

        // Outputs that just appeared set their own baseline.
        let outputs: Vec<&str> = next
            .outputs
            .iter()
            .map(|output| output.name.as_str())
            .filter(|name| previous.outputs.iter().any(|output| output.name == *name))
            .collect();

        let switched = |output: &&str| {
            let after = active_name(next, output);
            after.is_some() && after != active_name(&previous, output)
        };
        let focused = next.focused_output.as_deref();
        let switch = focused
            .filter(|output| outputs.contains(output) && switched(output))
            .or_else(|| outputs.iter().copied().find(switched));
        if let Some(output) = switch {
            return Some(Notice {
                output: output.to_owned(),
                reason: Reason::Switch,
                urgent: None,
            });
        }

        if let Some(output) = focused
            && previous.focused_output.is_some()
            && previous.focused_output.as_deref() != focused
            && outputs.contains(&output)
        {
            return Some(Notice {
                output: output.to_owned(),
                reason: Reason::Focus,
                urgent: None,
            });
        }

        for output in &outputs {
            let became_urgent = visible(next, output).find(|workspace| {
                workspace.urgent
                    && !visible(&previous, output)
                        .any(|before| before.name == workspace.name && before.urgent)
            });
            if let Some(workspace) = became_urgent {
                return Some(Notice {
                    output: (*output).to_owned(),
                    reason: Reason::Urgent,
                    urgent: Some(workspace.name.clone()),
                });
            }
        }

        let changed = outputs.iter().find(|output| {
            let names = |state| {
                visible(state, output)
                    .map(|workspace| &workspace.name)
                    .collect::<Vec<_>>()
            };
            names(&previous) != names(next)
        });
        changed.map(|output| Notice {
            output: (*output).to_owned(),
            reason: Reason::Changed,
            urgent: None,
        })
    }
}

/// What the view needs: the output's visible workspaces, which one is
/// active, and a monitor label when there is more than one monitor.
pub fn payload(notice: &Notice, state: &State, labels: &BTreeMap<String, String>) -> Value {
    let label = (state.outputs.len() > 1).then(|| {
        labels
            .get(&notice.output)
            .cloned()
            .unwrap_or_else(|| notice.output.clone())
    });
    let workspaces: Vec<Value> = visible(state, &notice.output)
        .map(|workspace| {
            json!({
                "name": workspace.name,
                "active": workspace.active,
                "urgent": workspace.urgent,
            })
        })
        .collect();
    json!({
        "output": notice.output,
        "label": label,
        "reason": notice.reason,
        "active": active_name(state, &notice.output),
        "urgent": notice.urgent,
        "workspaces": workspaces,
    })
}

fn visible<'a>(state: &'a State, output: &'a str) -> impl Iterator<Item = &'a Workspace> {
    state
        .workspaces_on(output)
        .filter(|workspace| !workspace.hidden)
}

fn active_name<'a>(state: &'a State, output: &str) -> Option<&'a str> {
    state
        .active_on(output)
        .map(|workspace| workspace.name.as_str())
}

#[cfg(test)]
mod tests {
    use mochi_core::compositor::{Output, WorkspaceId};

    use super::*;

    /// `spec` is `(output, name, flags)` with flags `a` active, `u` urgent and
    /// `h` hidden.
    fn state(outputs: &[&str], spec: &[(&str, &str, &str)]) -> State {
        State {
            backend: Backend::Wayland,
            outputs: outputs
                .iter()
                .map(|name| Output {
                    name: (*name).into(),
                    description: String::new(),
                })
                .collect(),
            workspaces: spec
                .iter()
                .zip(1..)
                .map(|((output, name, flags), id)| Workspace {
                    id: WorkspaceId(id),
                    name: (*name).into(),
                    output: Some((*output).into()),
                    coordinates: Vec::new(),
                    active: flags.contains('a'),
                    urgent: flags.contains('u'),
                    hidden: flags.contains('h'),
                    can_activate: true,
                })
                .collect(),
            focused_output: None,
        }
    }

    fn focused(mut state: State, output: &str) -> State {
        state.focused_output = Some(output.into());
        state
    }

    fn baseline(tracker: &mut Tracker) {
        tracker.apply(&state(
            &["DP-3", "HDMI-A-1"],
            &[
                ("DP-3", "1", "a"),
                ("DP-3", "2", ""),
                ("HDMI-A-1", "10", "a"),
            ],
        ));
    }

    #[test]
    fn the_first_snapshot_is_silent() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.apply(&state(&["DP-3"], &[("DP-3", "1", "a")])),
            None
        );
    }

    #[test]
    fn a_switch_names_the_output() {
        let mut tracker = Tracker::default();
        baseline(&mut tracker);
        let next = state(
            &["DP-3", "HDMI-A-1"],
            &[
                ("DP-3", "1", ""),
                ("DP-3", "2", "a"),
                ("HDMI-A-1", "10", "a"),
            ],
        );
        assert_eq!(
            tracker.apply(&next),
            Some(Notice {
                output: "DP-3".into(),
                reason: Reason::Switch,
                urgent: None
            })
        );
        assert_eq!(tracker.apply(&next), None);
    }

    #[test]
    fn switching_to_a_new_workspace_is_a_switch_not_a_change() {
        let mut tracker = Tracker::default();
        baseline(&mut tracker);
        let next = state(
            &["DP-3", "HDMI-A-1"],
            &[
                ("DP-3", "1", ""),
                ("DP-3", "2", ""),
                ("DP-3", "3", "a"),
                ("HDMI-A-1", "10", "a"),
            ],
        );
        assert_eq!(tracker.apply(&next).unwrap().reason, Reason::Switch);
    }

    #[test]
    fn focusing_another_output_shows_that_output() {
        let mut tracker = Tracker::default();
        let both = |focus| {
            focused(
                state(
                    &["DP-3", "HDMI-A-1"],
                    &[("DP-3", "1", "a"), ("HDMI-A-1", "10", "a")],
                ),
                focus,
            )
        };
        tracker.apply(&both("DP-3"));
        assert_eq!(
            tracker.apply(&both("HDMI-A-1")),
            Some(Notice {
                output: "HDMI-A-1".into(),
                reason: Reason::Focus,
                urgent: None
            })
        );
        assert_eq!(tracker.apply(&both("HDMI-A-1")), None);
    }

    #[test]
    fn focus_becoming_known_is_not_a_change() {
        let mut tracker = Tracker::default();
        baseline(&mut tracker);
        let next = focused(
            state(
                &["DP-3", "HDMI-A-1"],
                &[
                    ("DP-3", "1", "a"),
                    ("DP-3", "2", ""),
                    ("HDMI-A-1", "10", "a"),
                ],
            ),
            "DP-3",
        );
        assert_eq!(tracker.apply(&next), None);
    }

    #[test]
    fn a_switch_on_the_focused_output_wins() {
        let mut tracker = Tracker::default();
        tracker.apply(&focused(
            state(
                &["DP-3", "HDMI-A-1"],
                &[
                    ("DP-3", "1", "a"),
                    ("DP-3", "2", ""),
                    ("HDMI-A-1", "9", "a"),
                    ("HDMI-A-1", "10", ""),
                ],
            ),
            "DP-3",
        ));
        // Both outputs switch and focus moves to HDMI-A-1.
        let next = focused(
            state(
                &["DP-3", "HDMI-A-1"],
                &[
                    ("DP-3", "1", ""),
                    ("DP-3", "2", "a"),
                    ("HDMI-A-1", "9", ""),
                    ("HDMI-A-1", "10", "a"),
                ],
            ),
            "HDMI-A-1",
        );
        let notice = tracker.apply(&next).unwrap();
        assert_eq!(
            (notice.output.as_str(), notice.reason),
            ("HDMI-A-1", Reason::Switch)
        );
    }

    #[test]
    fn a_workspace_becoming_urgent_is_reported_once() {
        let mut tracker = Tracker::default();
        baseline(&mut tracker);
        let next = state(
            &["DP-3", "HDMI-A-1"],
            &[
                ("DP-3", "1", "a"),
                ("DP-3", "2", "u"),
                ("HDMI-A-1", "10", "a"),
            ],
        );
        assert_eq!(
            tracker.apply(&next),
            Some(Notice {
                output: "DP-3".into(),
                reason: Reason::Urgent,
                urgent: Some("2".into())
            })
        );
        assert_eq!(tracker.apply(&next), None);
    }

    #[test]
    fn removing_a_workspace_is_a_change() {
        let mut tracker = Tracker::default();
        baseline(&mut tracker);
        let next = state(
            &["DP-3", "HDMI-A-1"],
            &[("DP-3", "1", "a"), ("HDMI-A-1", "10", "a")],
        );
        assert_eq!(tracker.apply(&next).unwrap().reason, Reason::Changed);
    }

    #[test]
    fn hidden_workspaces_are_ignored() {
        let mut tracker = Tracker::default();
        baseline(&mut tracker);
        let next = state(
            &["DP-3", "HDMI-A-1"],
            &[
                ("DP-3", "1", "a"),
                ("DP-3", "2", ""),
                ("DP-3", "special", "hu"),
                ("HDMI-A-1", "10", "a"),
            ],
        );
        assert_eq!(tracker.apply(&next), None);
    }

    #[test]
    fn a_new_output_sets_its_own_baseline() {
        let mut tracker = Tracker::default();
        tracker.apply(&state(&["DP-3"], &[("DP-3", "1", "a")]));
        let next = state(
            &["DP-3", "HDMI-A-1"],
            &[("DP-3", "1", "a"), ("HDMI-A-1", "10", "a")],
        );
        assert_eq!(tracker.apply(&next), None);
    }

    #[test]
    fn the_compositor_becoming_available_sets_the_baseline() {
        let mut tracker = Tracker::default();
        tracker.apply(&State::default());
        assert_eq!(
            tracker.apply(&state(&["DP-3"], &[("DP-3", "1", "a")])),
            None
        );
    }

    #[test]
    fn the_payload_lists_visible_workspaces_and_labels_only_with_two_outputs() {
        let two = state(
            &["DP-3", "HDMI-A-1"],
            &[
                ("DP-3", "1", ""),
                ("DP-3", "2", "a"),
                ("DP-3", "special", "h"),
            ],
        );
        let notice = Notice {
            output: "DP-3".into(),
            reason: Reason::Switch,
            urgent: None,
        };
        let labels = BTreeMap::from([("DP-3".to_owned(), "Left".to_owned())]);

        assert_eq!(
            payload(&notice, &two, &labels),
            json!({
                "output": "DP-3",
                "label": "Left",
                "reason": "switch",
                "active": "2",
                "urgent": null,
                "workspaces": [
                    { "name": "1", "active": false, "urgent": false },
                    { "name": "2", "active": true, "urgent": false },
                ],
            })
        );

        assert_eq!(payload(&notice, &two, &BTreeMap::new())["label"], "DP-3");

        let one = state(&["DP-3"], &[("DP-3", "1", "a")]);
        assert_eq!(payload(&notice, &one, &labels)["label"], Value::Null);
    }
}

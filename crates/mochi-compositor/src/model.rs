//! Keeps what the compositor reported, keyed by protocol object ids, and
//! turns it into [`State`] snapshots. It never touches Wayland itself, so the
//! rules are tested without a compositor.

use std::collections::BTreeMap;

use crate::{Backend, Output, State, Workspace, WorkspaceId};

/// `ext_workspace_handle_v1.state` bits.
const ACTIVE: u32 = 1;
const URGENT: u32 = 2;
const HIDDEN: u32 = 4;
/// `ext_workspace_handle_v1.capabilities` bits.
const CAN_ACTIVATE: u32 = 1;

#[derive(Debug, Default)]
pub(crate) struct Model {
    outputs: BTreeMap<u32, OutputInfo>,
    groups: BTreeMap<u32, Group>,
    workspaces: BTreeMap<u32, WorkspaceInfo>,
    toplevels: BTreeMap<u32, Toplevel>,
    /// The output of the window that was focused last.
    window_focus: Option<u32>,
    /// The focused output from compositor IPC, by name. Exact, so it wins
    /// over the window guess.
    ipc_focus: Option<String>,
    /// Screencasts IPC reported started and not yet stopped.
    screencasts: u32,
}

#[derive(Debug, Default)]
struct Toplevel {
    outputs: Vec<u32>,
    activated: bool,
}

#[derive(Debug, Default)]
struct OutputInfo {
    name: String,
    description: String,
}

#[derive(Debug, Default)]
struct Group {
    outputs: Vec<u32>,
    workspaces: Vec<u32>,
}

#[derive(Debug, Default)]
struct WorkspaceInfo {
    name: String,
    coordinates: Vec<u32>,
    state: u32,
    capabilities: u32,
}

impl Model {
    pub fn output_name(&mut self, output: u32, name: String) {
        self.outputs.entry(output).or_default().name = name;
    }

    pub fn output_description(&mut self, output: u32, description: String) {
        self.outputs.entry(output).or_default().description = description;
    }

    pub fn output_removed(&mut self, output: u32) {
        self.outputs.remove(&output);
        for group in self.groups.values_mut() {
            group.outputs.retain(|id| *id != output);
        }
    }

    pub fn group_added(&mut self, group: u32) {
        self.groups.entry(group).or_default();
    }

    pub fn group_output(&mut self, group: u32, output: u32, entered: bool) {
        let outputs = &mut self.groups.entry(group).or_default().outputs;
        outputs.retain(|id| *id != output);
        if entered {
            outputs.push(output);
        }
    }

    pub fn group_workspace(&mut self, group: u32, workspace: u32, entered: bool) {
        let workspaces = &mut self.groups.entry(group).or_default().workspaces;
        workspaces.retain(|id| *id != workspace);
        if entered {
            workspaces.push(workspace);
        }
    }

    pub fn group_removed(&mut self, group: u32) {
        self.groups.remove(&group);
    }

    pub fn workspace_added(&mut self, workspace: u32) {
        self.workspaces.entry(workspace).or_default();
    }

    pub fn workspace_name(&mut self, workspace: u32, name: String) {
        self.workspaces.entry(workspace).or_default().name = name;
    }

    pub fn workspace_coordinates(&mut self, workspace: u32, coordinates: Vec<u32>) {
        self.workspaces.entry(workspace).or_default().coordinates = coordinates;
    }

    pub fn workspace_state(&mut self, workspace: u32, state: u32) {
        self.workspaces.entry(workspace).or_default().state = state;
    }

    pub fn workspace_capabilities(&mut self, workspace: u32, capabilities: u32) {
        self.workspaces.entry(workspace).or_default().capabilities = capabilities;
    }

    pub fn workspace_removed(&mut self, workspace: u32) {
        self.workspaces.remove(&workspace);
        for group in self.groups.values_mut() {
            group.workspaces.retain(|id| *id != workspace);
        }
    }

    pub fn toplevel_output(&mut self, toplevel: u32, output: u32, entered: bool) {
        let outputs = &mut self.toplevels.entry(toplevel).or_default().outputs;
        outputs.retain(|id| *id != output);
        if entered {
            outputs.push(output);
        }
    }

    pub fn toplevel_activated(&mut self, toplevel: u32, activated: bool) {
        self.toplevels.entry(toplevel).or_default().activated = activated;
    }

    /// The window's state is complete. A focused window moves the focus to
    /// its output. A window losing focus doesn't: focus went somewhere else,
    /// which that window's own `done` reports.
    pub fn toplevel_done(&mut self, toplevel: u32) {
        if let Some(window) = self.toplevels.get(&toplevel)
            && window.activated
            && let Some(output) = window.outputs.first()
        {
            self.window_focus = Some(*output);
        }
    }

    pub fn toplevel_closed(&mut self, toplevel: u32) {
        self.toplevels.remove(&toplevel);
    }

    pub fn ipc_focus(&mut self, output: String) {
        self.ipc_focus = Some(output);
    }

    /// One screencast started or stopped. Hyprland reports each capture
    /// session, so several can run at once.
    pub fn ipc_screencast(&mut self, started: bool) {
        self.screencasts = if started {
            self.screencasts.saturating_add(1)
        } else {
            self.screencasts.saturating_sub(1)
        };
    }

    /// After reconnecting to IPC: whatever was counted may be stale.
    pub fn reset_screencasts(&mut self) {
        self.screencasts = 0;
    }

    fn focused_output(&self) -> Option<String> {
        self.ipc_focus.clone().or_else(|| {
            self.window_focus
                .and_then(|output| self.outputs.get(&output))
                .map(|output| output.name.clone())
                .filter(|name| !name.is_empty())
        })
    }

    /// Outputs by name, and workspaces grouped by output in the order a user
    /// expects: by coordinates, then by the number their name starts with,
    /// then by name.
    pub fn snapshot(&self) -> State {
        let mut outputs = self
            .outputs
            .values()
            .filter(|output| !output.name.is_empty())
            .map(|output| Output {
                name: output.name.clone(),
                description: output.description.clone(),
            })
            .collect::<Vec<_>>();

        let mut workspaces: Vec<Workspace> = self
            .workspaces
            .iter()
            .map(|(id, info)| Workspace {
                id: WorkspaceId(*id),
                name: info.name.clone(),
                output: self.output_of(*id),
                coordinates: info.coordinates.clone(),
                active: info.state & ACTIVE != 0,
                urgent: info.state & URGENT != 0,
                hidden: info.state & HIDDEN != 0,
                can_activate: info.capabilities & CAN_ACTIVATE != 0,
            })
            .collect();
        workspaces.sort_by_cached_key(|workspace| {
            (
                // Workspaces not on any output go last.
                workspace.output.is_none(),
                workspace.output.clone(),
                workspace.coordinates.clone(),
                leading_number(&workspace.name),
                workspace.name.clone(),
            )
        });

        outputs.sort_by(|a, b| a.name.cmp(&b.name));
        State {
            backend: Backend::Wayland,
            outputs,
            workspaces,
            focused_output: self.focused_output(),
            screencast: self.screencasts > 0,
        }
    }

    /// The first output of the group holding the workspace.
    fn output_of(&self, workspace: u32) -> Option<String> {
        self.groups
            .values()
            .find(|group| group.workspaces.contains(&workspace))
            .and_then(|group| group.outputs.first())
            .and_then(|output| self.outputs.get(output))
            .map(|output| output.name.clone())
            .filter(|name| !name.is_empty())
    }
}

/// `"10: web"` sorts after `"9"`. Names without a number go last.
fn leading_number(name: &str) -> u64 {
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two outputs, each with a group, like a dual-monitor Hyprland session.
    fn dual_monitor() -> Model {
        let mut model = Model::default();
        model.output_name(1, "DP-3".into());
        model.output_description(1, "Samsung C27F390".into());
        model.output_name(2, "HDMI-A-1".into());

        model.group_added(10);
        model.group_output(10, 1, true);
        model.group_added(11);
        model.group_output(11, 2, true);

        for (id, name, group) in [(20, "10", 10), (21, "2", 10), (22, "1", 11)] {
            model.workspace_added(id);
            model.workspace_name(id, name.into());
            model.workspace_capabilities(id, CAN_ACTIVATE);
            model.group_workspace(group, id, true);
        }
        model.workspace_state(21, ACTIVE);
        model.workspace_state(22, ACTIVE | URGENT);
        model
    }

    fn names(state: &State) -> Vec<(&str, Option<&str>)> {
        state
            .workspaces
            .iter()
            .map(|workspace| (workspace.name.as_str(), workspace.output.as_deref()))
            .collect()
    }

    #[test]
    fn workspaces_are_grouped_by_output_and_sorted_by_number() {
        let state = dual_monitor().snapshot();
        assert_eq!(
            names(&state),
            [
                ("2", Some("DP-3")),
                ("10", Some("DP-3")),
                ("1", Some("HDMI-A-1"))
            ]
        );
        assert_eq!(state.outputs[0].description, "Samsung C27F390");
    }

    #[test]
    fn state_and_capability_bits_are_decoded() {
        let state = dual_monitor().snapshot();
        let on_hdmi = state.active_on("HDMI-A-1").unwrap();
        assert!(on_hdmi.active && on_hdmi.urgent && !on_hdmi.hidden && on_hdmi.can_activate);
        assert_eq!(state.active_on("DP-3").unwrap().name, "2");
        assert_eq!(state.active_on("eDP-1"), None);
    }

    #[test]
    fn moving_a_workspace_between_groups_changes_its_output() {
        let mut model = dual_monitor();
        model.group_workspace(10, 20, false);
        model.group_workspace(11, 20, true);
        let state = model.snapshot();
        let moved = state.workspaces.iter().find(|w| w.name == "10").unwrap();
        assert_eq!(moved.output.as_deref(), Some("HDMI-A-1"));
    }

    #[test]
    fn removed_workspaces_and_outputs_disappear() {
        let mut model = dual_monitor();
        model.workspace_removed(21);
        model.output_removed(2);
        let state = model.snapshot();
        assert_eq!(names(&state), [("10", Some("DP-3")), ("1", None)]);
        assert_eq!(state.outputs.len(), 1);
    }

    #[test]
    fn coordinates_sort_before_names() {
        let mut model = Model::default();
        for (id, name, x) in [(1, "b", 0), (2, "a", 1)] {
            model.workspace_added(id);
            model.workspace_name(id, name.into());
            model.workspace_coordinates(id, vec![x]);
        }
        let state = model.snapshot();
        assert_eq!(state.workspaces[0].name, "b");
    }

    #[test]
    fn outputs_without_a_name_yet_are_left_out() {
        let mut model = Model::default();
        model.output_description(1, "Not named yet".into());
        assert!(model.snapshot().outputs.is_empty());
    }

    #[test]
    fn counts_screencasts_until_each_stops() {
        let mut model = Model::default();
        assert!(!model.snapshot().screencast);
        // Two thumbnails start capturing, one stops: still capturing.
        model.ipc_screencast(true);
        model.ipc_screencast(true);
        model.ipc_screencast(false);
        assert!(model.snapshot().screencast);
        model.ipc_screencast(false);
        assert!(!model.snapshot().screencast);
        // A stop without a start never goes below zero.
        model.ipc_screencast(false);
        model.ipc_screencast(true);
        assert!(model.snapshot().screencast);
        model.reset_screencasts();
        assert!(!model.snapshot().screencast);
    }

    #[test]
    fn the_focused_window_gives_the_focused_output() {
        let mut model = dual_monitor();
        assert_eq!(model.snapshot().focused_output, None);

        model.toplevel_output(30, 2, true);
        model.toplevel_activated(30, true);
        model.toplevel_done(30);
        assert_eq!(model.snapshot().focused_output.as_deref(), Some("HDMI-A-1"));

        // Losing focus alone moves nothing; the next focused window does.
        model.toplevel_activated(30, false);
        model.toplevel_done(30);
        assert_eq!(model.snapshot().focused_output.as_deref(), Some("HDMI-A-1"));
        model.toplevel_output(31, 1, true);
        model.toplevel_activated(31, true);
        model.toplevel_done(31);
        assert_eq!(model.snapshot().focused_output.as_deref(), Some("DP-3"));
    }

    #[test]
    fn a_window_focused_before_it_enters_an_output_counts_once_it_does() {
        let mut model = dual_monitor();
        model.toplevel_activated(30, true);
        model.toplevel_done(30);
        assert_eq!(model.snapshot().focused_output, None);
        model.toplevel_output(30, 1, true);
        model.toplevel_done(30);
        assert_eq!(model.snapshot().focused_output.as_deref(), Some("DP-3"));
    }

    #[test]
    fn ipc_focus_wins_over_the_window_guess() {
        let mut model = dual_monitor();
        model.toplevel_output(30, 1, true);
        model.toplevel_activated(30, true);
        model.toplevel_done(30);
        // Focusing an empty workspace on the other output: no window gets
        // focus, but the compositor's IPC says where focus is.
        model.ipc_focus("HDMI-A-1".into());
        assert_eq!(model.snapshot().focused_output.as_deref(), Some("HDMI-A-1"));
    }

    #[test]
    fn names_without_numbers_sort_last() {
        assert!(leading_number("3") < leading_number("10: web"));
        assert_eq!(leading_number("special"), u64::MAX);
    }
}

//! The conversation between mochid and a plugin backend.
//!
//! mochid starts the backend with one end of a socket pair as the file
//! descriptor named by [`FD_ENV`], and the same JSON lines as the main
//! socket go over it: mochid sends [`ToPlugin`], the backend sends
//! [`FromPlugin`]. mochid speaks first, with `hello`; the backend answers
//! with its own `hello`, then runs until mochid closes the socket.
//!
//! The messages mirror a builtin module's `ModuleCtx`, one message per
//! call. The backend numbers its activities and bubbles itself, from 1, and
//! events about them use its numbers. Requests that expect an answer carry
//! an `id` the answer repeats.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Contribution;
use crate::spec::{ActivitySpec, Args, BubbleSpec, CallError, EndReason};

/// The environment variable holding the backend's end of the socket, as a
/// file descriptor number: always `3`.
pub const FD_ENV: &str = "MOCHI_PLUGIN_FD";

/// The environment variable holding the plugin's directory, where its
/// manifest is. The backend also starts there.
pub const DIR_ENV: &str = "MOCHI_PLUGIN_DIR";

/// Sent by mochid to a plugin backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToPlugin {
    /// The first message. The backend answers with its own `hello`.
    Hello {
        api: u32,
        /// mochid's version.
        version: String,
        /// The plugin's id.
        module: String,
        /// Its `[module.<id>]` table from config.toml, as JSON.
        settings: Value,
        /// See `ModuleCtx::data_dir`.
        data_dir: PathBuf,
        /// See `ModuleCtx::session_dir`.
        session_dir: PathBuf,
        compositor: CompositorState,
    },
    /// Runs one of its actions, with arguments already checked against the
    /// manifest. Answer with `reply` and the same `id`.
    Command { id: u64, action: String, args: Args },
    /// A click on one of its activities that has no expanded view.
    Clicked { activity: u64 },
    /// The pointer came onto one of its activities on the island, or left.
    Hovered { activity: u64, hovered: bool },
    /// One of its activities is gone for good.
    Ended { activity: u64, reason: EndReason },
    /// A click on one of its bubbles.
    BubbleClicked { bubble: u64 },
    /// A module named in the manifest's `[uses] state` published state:
    /// `null` when it stopped. Sent once at the start when it has some.
    State { module: String, state: Value },
    /// The compositor's state changed.
    Compositor { state: CompositorState },
    /// What the enabled modules offer this plugin: every contribution whose
    /// `target` is its id. Sent at the start, then when a reload changes it.
    Offers { offers: Vec<Contribution> },
    /// The answer to a `call`.
    CallResult {
        id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<CallError>,
        /// What the action answered with, like the output `mochi ipc`
        /// prints.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// The answer to a compositor request: `windows` returns a list of
    /// [`WindowInfo`], `pointer_output` a name or `null`.
    CompositorResult {
        id: u64,
        #[serde(default)]
        value: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// Sent by a plugin backend to mochid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FromPlugin {
    /// The answer to mochid's `hello`, with the protocol version the
    /// backend speaks.
    Hello {
        api: u32,
    },
    /// Replaces its state, which the UI and other modules can read.
    PublishState {
        state: Value,
    },
    /// Submits an activity, numbered by the backend.
    Present {
        id: u64,
        spec: ActivitySpec,
    },
    /// Replaces an activity's payload.
    Update {
        id: u64,
        payload: Value,
    },
    Withdraw {
        id: u64,
    },
    /// Shows a bubble, numbered by the backend, or replaces the one with the
    /// same key.
    ShowBubble {
        id: u64,
        spec: BubbleSpec,
    },
    UpdateBubble {
        id: u64,
        payload: Value,
    },
    HideBubble {
        id: u64,
    },
    /// Answers a `command`: with `output` to print, or `error`, or neither
    /// for plain success.
    Reply {
        id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Runs another module's action. Answered with `call_result`.
    Call {
        id: u64,
        module: String,
        action: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Switches to a workspace. Answered with `compositor_result`.
    ActivateWorkspace {
        id: u64,
        workspace: u32,
    },
    /// Asks for the windows on visible workspaces. Answered with
    /// `compositor_result`.
    Windows {
        id: u64,
    },
    /// Asks which output the pointer is on. Answered with
    /// `compositor_result`.
    PointerOutput {
        id: u64,
    },
}

/// The compositor's state, as `ModuleCtx::compositor` gives builtins.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CompositorState {
    /// `wayland`, or `unsupported` without compositor information.
    pub backend: String,
    /// Sorted by name.
    pub outputs: Vec<OutputInfo>,
    /// Grouped by output, each output's in display order.
    pub workspaces: Vec<WorkspaceInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The output with keyboard focus, by name, when the compositor says.
    pub focused_output: Option<String>,
    /// The app id of the window that had keyboard focus last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_app: Option<String>,
    /// Something is capturing the screen.
    #[serde(default)]
    pub screencast: bool,
    /// What is being captured, by monitor or window name.
    #[serde(default)]
    pub captured: Vec<String>,
    /// The active keyboard layout's name, like `English (US)`, when the
    /// compositor says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard_layout: Option<String>,
}

/// A monitor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputInfo {
    /// The connector name, like `DP-3`.
    pub name: String,
    /// A human-readable name, often the monitor's model.
    pub description: String,
    /// Its mode in pixels, 0 until the compositor says.
    pub width: u32,
    /// See `width`.
    pub height: u32,
}

/// A workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// Valid while the workspace exists; compositors may reuse it after.
    pub id: u32,
    /// Its name, like `1`.
    pub name: String,
    /// The output it's on, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Its position in the compositor's layout, when it has one.
    #[serde(default)]
    pub coordinates: Vec<u32>,
    /// Shown on its output.
    pub active: bool,
    /// Something on it wants attention.
    #[serde(default)]
    pub urgent: bool,
    /// The compositor hides it from workspace lists.
    #[serde(default)]
    pub hidden: bool,
    /// Whether `activate_workspace` may switch to it.
    #[serde(default)]
    pub can_activate: bool,
}

/// A window on a visible workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Its title.
    pub title: String,
    /// Its app id, like `firefox`.
    pub app_id: String,
    /// Its position in the compositor's global layout, in logical pixels.
    pub x: i32,
    /// See `x`.
    pub y: i32,
    /// Its size, in logical pixels.
    pub width: i32,
    /// See `width`.
    pub height: i32,
    /// Floating rather than tiled.
    pub floating: bool,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::spec::ArgValue;
    use crate::{decode, encode};

    fn round_trip_to(message: ToPlugin) {
        let line = encode(&message).unwrap();
        let back: ToPlugin = decode(std::str::from_utf8(&line).unwrap()).unwrap();
        assert_eq!(back, message);
    }

    fn round_trip_from(message: FromPlugin) {
        let line = encode(&message).unwrap();
        let back: FromPlugin = decode(std::str::from_utf8(&line).unwrap()).unwrap();
        assert_eq!(back, message);
    }

    #[test]
    fn every_message_to_a_plugin_round_trips() {
        round_trip_to(ToPlugin::Hello {
            api: 1,
            version: "0.1.2".into(),
            module: "pomodoro".into(),
            settings: json!({ "minutes": 25 }),
            data_dir: "/run/user/1000/mochi/data/pomodoro".into(),
            session_dir: "/run/user/1000/mochi/session/pomodoro".into(),
            compositor: CompositorState::default(),
        });
        round_trip_to(ToPlugin::Command {
            id: 1,
            action: "start".into(),
            args: [("minutes".to_owned(), ArgValue::Int(5))]
                .into_iter()
                .collect(),
        });
        round_trip_to(ToPlugin::Clicked { activity: 2 });
        round_trip_to(ToPlugin::Hovered {
            activity: 2,
            hovered: true,
        });
        round_trip_to(ToPlugin::Ended {
            activity: 2,
            reason: EndReason::Dismissed,
        });
        round_trip_to(ToPlugin::BubbleClicked { bubble: 3 });
        round_trip_to(ToPlugin::State {
            module: "media".into(),
            state: json!({ "playing": true }),
        });
        round_trip_to(ToPlugin::Compositor {
            state: CompositorState {
                backend: "wayland".into(),
                outputs: vec![OutputInfo {
                    name: "DP-3".into(),
                    description: "Some monitor".into(),
                    width: 1920,
                    height: 1080,
                }],
                workspaces: vec![WorkspaceInfo {
                    id: 4,
                    name: "1".into(),
                    output: Some("DP-3".into()),
                    coordinates: vec![0],
                    active: true,
                    urgent: false,
                    hidden: false,
                    can_activate: true,
                }],
                focused_output: Some("DP-3".into()),
                focused_app: Some("kitty".into()),
                screencast: false,
                captured: Vec::new(),
                keyboard_layout: Some("English (US)".into()),
            },
        });
        round_trip_to(ToPlugin::CallResult {
            id: 4,
            error: None,
            output: None,
        });
        round_trip_to(ToPlugin::CallResult {
            id: 5,
            error: Some(CallError::NotEnabled("media".into())),
            output: None,
        });
        round_trip_to(ToPlugin::CallResult {
            id: 6,
            error: None,
            output: Some("{\"title\": \"4\"}".into()),
        });
        round_trip_to(ToPlugin::Offers {
            offers: vec![Contribution {
                module: "emoji".into(),
                target: "launcher".into(),
                kind: "provider".into(),
                id: "emoji".into(),
                view: String::new(),
                title: "Emoji".into(),
                icon: None,
                order: 0,
                options: json!({ "prefix": ":" }),
            }],
        });
        round_trip_to(ToPlugin::CompositorResult {
            id: 6,
            value: json!("DP-3"),
            error: None,
        });
    }

    #[test]
    fn every_message_from_a_plugin_round_trips() {
        round_trip_from(FromPlugin::Hello { api: 1 });
        round_trip_from(FromPlugin::PublishState {
            state: json!({ "left": 300 }),
        });
        round_trip_from(FromPlugin::Present {
            id: 1,
            spec: ActivitySpec::new("Timer").key("timer"),
        });
        round_trip_from(FromPlugin::Update {
            id: 1,
            payload: json!({ "left": 299 }),
        });
        round_trip_from(FromPlugin::Withdraw { id: 1 });
        round_trip_from(FromPlugin::ShowBubble {
            id: 2,
            spec: BubbleSpec::new("Dot"),
        });
        round_trip_from(FromPlugin::UpdateBubble {
            id: 2,
            payload: json!(1),
        });
        round_trip_from(FromPlugin::HideBubble { id: 2 });
        round_trip_from(FromPlugin::Reply {
            id: 1,
            output: Some("done".into()),
            error: None,
        });
        round_trip_from(FromPlugin::Call {
            id: 3,
            module: "media".into(),
            action: "toggle".into(),
            args: Vec::new(),
        });
        round_trip_from(FromPlugin::ActivateWorkspace {
            id: 4,
            workspace: 2,
        });
        round_trip_from(FromPlugin::Windows { id: 5 });
        round_trip_from(FromPlugin::PointerOutput { id: 6 });
    }

    /// Plugins in other languages write these by hand.
    #[test]
    fn short_forms_parse() {
        let present: FromPlugin =
            decode(r#"{"type":"present","id":1,"spec":{"compact":"Timer","timeout_ms":3000}}"#)
                .unwrap();
        let FromPlugin::Present { spec, .. } = present else {
            panic!("not a present");
        };
        assert_eq!(spec.timeout, Some(std::time::Duration::from_secs(3)));

        let reply: FromPlugin = decode(r#"{"type":"reply","id":1}"#).unwrap();
        assert_eq!(
            reply,
            FromPlugin::Reply {
                id: 1,
                output: None,
                error: None
            }
        );
    }
}

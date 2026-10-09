use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Area, Bubble, BubbleId, Contribution, ModuleActions, Overflow, Stacking, Theme};

/// Identifies one activity for its whole life, across the daemon and the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActivityId(pub u64);

impl fmt::Display for ActivityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Who is on the other end of a connection. It decides which messages the
/// daemon accepts from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The Quickshell UI.
    Ui,
    /// The `mochi` CLI or any other control client.
    Ctl,
}

/// Sent to the daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Must be the first message on every connection.
    Hello { api: u32, role: Role },
    /// Runs a module action. Answered with `ok` or `error`.
    Command {
        module: String,
        action: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Something happened to the island. UI only, never answered.
    Event {
        activity: ActivityId,
        kind: EventKind,
        /// The monitor whose island it happened on, when the UI knows.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// For `outside`: the click that closed it, which the daemon passes
        /// on to the window under it once the island lets go.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        click: Option<Click>,
    },
    /// A scroll the island caught that belongs to the window under it, once
    /// the island lets go there. UI only, never answered.
    PassOn { click: Click },
    /// A click on a bubble. UI only, never answered.
    BubbleClick { bubble: BubbleId },
    /// A click on an area's "+N", for the bubbles it leaves out. The daemon
    /// lists them in the island. UI only, never answered.
    OverflowClick { area: Area },
    /// Answered with `status`.
    Status,
    /// Reloads config and theme. Answered with `ok` or `error`.
    Reload,
    /// Closes what the island shows, as a right click on it does. For a
    /// keybind: notices that don't take the keyboard can't hear Escape.
    /// Answered with `ok`, also when nothing shows.
    Dismiss,
    /// Answered with `actions`, for one module or all of them.
    ListActions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        module: Option<String>,
    },
}

/// Sent by the daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonMessage {
    /// Answer to the client's `hello`.
    Hello { api: u32, version: String },
    /// The enabled modules. Sent to the UI after `hello` and when the list
    /// changes.
    Modules { modules: Vec<String> },
    /// What modules offer each other, from every enabled module. Sent to the
    /// UI after `hello`.
    Contributions { contributions: Vec<Contribution> },
    /// The latest state a module published. Sent to the UI after `hello` for
    /// every module, then whenever it changes.
    State { module: String, state: Value },
    /// A passing value a module sends many times a second, like an audio
    /// meter's level. The daemon doesn't keep it or send it again.
    Live { module: String, value: Value },
    /// What the island shows now. `None` when no activity exists at all.
    Present {
        activity: Option<Activity>,
        /// What islands on other monitors show while `activity` is meant
        /// for one monitor: the idle island, when it runs. Only without
        /// `output`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resting: Option<Activity>,
        /// The monitor whose island shows `activity`; every monitor's
        /// without it. Each monitor has an island of its own.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// Every bubble, in drawing order. Sent to the UI after `hello` and on
    /// every change.
    Bubbles {
        bubbles: Vec<Bubble>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        overflow: Vec<Overflow>,
        /// Each area stacks its bubbles into one, when set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stack: Option<Stacking>,
        /// How long the pointer rests on a bubble before its tooltip shows;
        /// none means never.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tooltip_ms: Option<u64>,
    },
    /// Design tokens. Sent to the UI after `hello` and on reload.
    Theme { theme: Box<Theme> },
    /// A request succeeded.
    Ok,
    /// A command succeeded with something to say: a choice the user made,
    /// for example. `mochi ipc` prints it.
    Output { output: String },
    /// Answer to `status`.
    Status { status: Status },
    /// Answer to `list_actions`.
    Actions { modules: Vec<ModuleActions> },
    /// A request failed, or a message could not be handled.
    Error { code: ErrorCode, message: String },
}

/// The activity the island shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub id: ActivityId,
    pub module: String,
    /// The view to load: `modules/<module>/<view>.qml`.
    pub view: String,
    pub payload: Value,
    /// Whether `view` is the expanded view.
    pub expanded: bool,
    /// Whether a click toggles between a compact and an expanded view.
    pub expandable: bool,
    /// The module's replacement key. A new activity with the same module, key
    /// and view continues the previous one: the UI updates the payload in
    /// place instead of switching views.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Takes the keyboard while shown, and a click anywhere outside the island
    /// dismisses it. For views you type into, like a launcher.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub modal: bool,
    /// A full-screen view, `modules/<module>/<overlay>.qml`, drawn under the
    /// island on every monitor while the activity shows. For picking
    /// something on screen, like a region to capture. Implies `modal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
    /// The monitor a modal activity shows on, by name. The islands on other
    /// monitors keep what they showed. Without one, every island shows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// A click outside the island closes it, which means the UI catches
    /// every click while it shows. Implied by `modal`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub outside: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Click,
    HoverEnter,
    HoverLeave,
    /// The user closed the activity, for example with a swipe or a close
    /// button.
    Dismiss,
    /// The user clicked outside the island while the activity was shown.
    Outside,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// The daemon's version.
    pub version: String,
    pub api: u32,
    pub ui_connected: bool,
    pub modules: Vec<String>,
    #[serde(default)]
    pub compositor: CompositorStatus,
    /// The plugins in plugins.toml.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PluginStatus>,
}

/// One plugin from plugins.toml, as the daemon sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginStatus {
    pub id: String,
    pub state: PluginState,
    /// Why it's missing or failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Enabled and running.
    Running,
    /// Installed, but not in `modules`.
    Disabled,
    /// Listed but not installed, or its manifest has an error.
    Missing,
    /// Its backend kept crashing and was stopped until the next reload.
    Failed,
}

/// What the daemon knows about the compositor.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CompositorStatus {
    /// `wayland`, or `unsupported` when no compositor information is
    /// available.
    pub backend: String,
    /// Output names.
    pub outputs: Vec<String>,
    pub workspaces: usize,
    /// The output with focus, when the compositor says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The line was not a valid message.
    BadMessage,
    /// Something other than `hello` arrived first.
    HelloFirst,
    UnsupportedApi,
    /// The connection's role may not send this message.
    NotAllowed,
    UnknownModule,
    UnknownAction,
    InvalidArgs,
    /// The module accepted the command but it failed.
    ModuleFailed,
    /// `reload` found an error in a config file.
    InvalidConfig,
    Internal,
}

/// A click outside the island, where it happened, to pass on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Click {
    /// The monitor it happened on.
    pub output: String,
    /// Where, in that monitor's logical pixels, out of `width` by `height`.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Qt's button: 1 left, 2 right, 4 middle; 0 for a scroll.
    pub button: u32,
    /// A scroll instead of a click: Qt's wheel angles, 120 a notch,
    /// positive away from the user and to the right.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll: Option<Scroll>,
}

/// A wheel or touchpad scroll, as Qt reports it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scroll {
    pub x: f64,
    pub y: f64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{ActionSpec, ArgSpec, decode, encode};

    fn round_trip_client(message: ClientMessage) {
        let line = encode(&message).unwrap();
        let back: ClientMessage = decode(std::str::from_utf8(&line).unwrap()).unwrap();
        assert_eq!(back, message);
    }

    fn round_trip_daemon(message: DaemonMessage) {
        let line = encode(&message).unwrap();
        let back: DaemonMessage = decode(std::str::from_utf8(&line).unwrap()).unwrap();
        assert_eq!(back, message);
    }

    fn activity() -> Activity {
        Activity {
            id: ActivityId(7),
            module: "notifications".into(),
            view: "Compact".into(),
            payload: json!({ "title": "Hello" }),
            expanded: false,
            expandable: true,
            modal: false,
            overlay: None,
            output: None,
            outside: false,
            key: None,
        }
    }

    #[test]
    fn every_client_message_round_trips() {
        round_trip_client(ClientMessage::Hello {
            api: 1,
            role: Role::Ui,
        });
        round_trip_client(ClientMessage::Command {
            module: "launcher".into(),
            action: "toggle".into(),
            args: vec!["--fast".into()],
        });
        round_trip_client(ClientMessage::Event {
            activity: ActivityId(3),
            kind: EventKind::HoverLeave,
            output: None,
            click: None,
        });
        round_trip_client(ClientMessage::Event {
            activity: ActivityId(3),
            kind: EventKind::Click,
            output: Some("DP-3".into()),
            click: None,
        });
        round_trip_client(ClientMessage::Event {
            activity: ActivityId(3),
            kind: EventKind::Outside,
            output: Some("DP-3".into()),
            click: Some(Click {
                output: "HDMI-A-1".into(),
                x: 640.5,
                y: 300.0,
                width: 1920.0,
                height: 1080.0,
                button: 1,
                scroll: None,
            }),
        });
        round_trip_client(ClientMessage::PassOn {
            click: Click {
                output: "DP-3".into(),
                x: 10.0,
                y: 20.0,
                width: 1920.0,
                height: 1080.0,
                button: 0,
                scroll: Some(Scroll { x: 0.0, y: -120.0 }),
            },
        });
        round_trip_client(ClientMessage::BubbleClick {
            bubble: BubbleId(3),
        });
        round_trip_client(ClientMessage::OverflowClick { area: Area::Right });
        round_trip_client(ClientMessage::Status);
        round_trip_client(ClientMessage::Reload);
        round_trip_client(ClientMessage::Dismiss);
        round_trip_client(ClientMessage::ListActions { module: None });
        round_trip_client(ClientMessage::ListActions {
            module: Some("osd".into()),
        });
    }

    #[test]
    fn every_daemon_message_round_trips() {
        round_trip_daemon(DaemonMessage::Hello {
            api: 1,
            version: "0.0.8".into(),
        });
        round_trip_daemon(DaemonMessage::Modules {
            modules: vec!["idle".into()],
        });
        round_trip_daemon(DaemonMessage::State {
            module: "idle".into(),
            state: json!({ "unread": 2 }),
        });
        round_trip_daemon(DaemonMessage::Live {
            module: "audio".into(),
            value: json!({ "output": 0.5 }),
        });
        round_trip_daemon(DaemonMessage::Present {
            activity: Some(activity()),
            resting: None,
            output: None,
        });
        round_trip_daemon(DaemonMessage::Present {
            activity: Some(Activity {
                output: Some("DP-3".into()),
                ..activity()
            }),
            resting: Some(activity()),
            output: None,
        });
        round_trip_daemon(DaemonMessage::Present {
            activity: Some(Activity {
                modal: true,
                overlay: Some("Overlay".into()),
                ..activity()
            }),
            resting: None,
            output: None,
        });
        round_trip_daemon(DaemonMessage::Present {
            activity: Some(Activity {
                modal: true,
                output: Some("DP-3".into()),
                ..activity()
            }),
            resting: None,
            output: None,
        });
        round_trip_daemon(DaemonMessage::Present {
            activity: None,
            resting: None,
            output: None,
        });
        round_trip_daemon(DaemonMessage::Present {
            activity: Some(activity()),
            resting: None,
            output: Some("HDMI-A-1".into()),
        });
        round_trip_daemon(DaemonMessage::Output {
            output: "[SELECTION]/screen:DP-3".into(),
        });
        round_trip_daemon(DaemonMessage::Contributions {
            contributions: vec![Contribution {
                module: "media".into(),
                target: "hub".into(),
                kind: "card".into(),
                id: "now-playing".into(),
                view: "Card".into(),
                title: "Now playing".into(),
                icon: None,
                order: 10,
                options: json!({ "span": 2 }),
            }],
        });
        round_trip_daemon(DaemonMessage::Bubbles {
            bubbles: vec![Bubble {
                id: BubbleId(4),
                module: "media".into(),
                key: Some("now".into()),
                view: "Bubble".into(),
                wide: true,
                payload: json!({ "title": "Song" }),
                area: Area::CenterLeft,
                group: Some("status".into()),
                priority: 50,
                news: 3,
                tooltip: Some("Song · Artist".into()),
            }],
            overflow: vec![Overflow {
                area: Area::Right,
                hidden: 2,
            }],
            stack: Some(Stacking { news_ms: 4000 }),
            tooltip_ms: Some(600),
        });
        round_trip_daemon(DaemonMessage::Theme {
            theme: Box::default(),
        });
        round_trip_daemon(DaemonMessage::Ok);
        round_trip_daemon(DaemonMessage::Status {
            status: Status {
                version: "0.0.8".into(),
                api: 1,
                ui_connected: true,
                modules: vec!["idle".into()],
                compositor: CompositorStatus {
                    backend: "wayland".into(),
                    outputs: vec!["DP-3".into()],
                    workspaces: 4,
                    focused: Some("DP-3".into()),
                },
                plugins: vec![PluginStatus {
                    id: "pomodoro".into(),
                    state: PluginState::Failed,
                    message: Some("it crashed 5 times in a minute".into()),
                }],
            },
        });
        round_trip_daemon(DaemonMessage::Actions {
            modules: vec![ModuleActions {
                module: "osd".into(),
                actions: vec![
                    ActionSpec::new("volume", "Change the volume")
                        .arg(ArgSpec::int("delta", "Percent to add or remove")),
                ],
            }],
        });
        round_trip_daemon(DaemonMessage::Error {
            code: ErrorCode::UnknownAction,
            message: "osd has no action nope".into(),
        });
    }

    /// The UI parses these by hand, so their exact shape is part of the API.
    #[test]
    fn ui_facing_messages_keep_their_wire_format() {
        let present = DaemonMessage::Present {
            activity: Some(activity()),
            resting: None,
            output: None,
        };
        assert_eq!(
            serde_json::to_value(&present).unwrap(),
            json!({
                "type": "present",
                "activity": {
                    "id": 7,
                    "module": "notifications",
                    "view": "Compact",
                    "payload": { "title": "Hello" },
                    "expanded": false,
                    "expandable": true
                }
            })
        );

        let event: ClientMessage =
            decode(r#"{"type":"event","activity":7,"kind":"hover_enter"}"#).unwrap();
        assert_eq!(
            event,
            ClientMessage::Event {
                activity: ActivityId(7),
                kind: EventKind::HoverEnter,
                output: None,
                click: None,
            }
        );

        let overflow: ClientMessage =
            decode(r#"{"type":"overflow_click","area":"center-right"}"#).unwrap();
        assert_eq!(
            overflow,
            ClientMessage::OverflowClick {
                area: Area::CenterRight
            }
        );
    }

    #[test]
    fn the_key_is_only_sent_when_set() {
        let keyed = Activity {
            key: Some("osd".into()),
            ..activity()
        };
        let json = serde_json::to_value(&keyed).unwrap();
        assert_eq!(json["key"], "osd");

        let back: Activity = serde_json::from_value(json).unwrap();
        assert_eq!(back, keyed);

        let without: Activity = serde_json::from_str(
            r#"{"id":1,"module":"idle","view":"Pill","payload":{},"expanded":false,"expandable":false}"#,
        )
        .unwrap();
        assert_eq!(without.key, None);
    }

    #[test]
    fn optional_fields_may_be_left_out() {
        let command: ClientMessage =
            decode(r#"{"type":"command","module":"idle","action":"show"}"#).unwrap();
        assert!(matches!(command, ClientMessage::Command { args, .. } if args.is_empty()));

        let list: ClientMessage = decode(r#"{"type":"list_actions"}"#).unwrap();
        assert_eq!(list, ClientMessage::ListActions { module: None });
    }

    #[test]
    fn unknown_message_types_are_rejected() {
        assert!(decode::<ClientMessage>(r#"{"type":"teleport"}"#).is_err());
        assert!(decode::<ClientMessage>(r#"{"api":1}"#).is_err());
    }
}

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Bubble, BubbleId, ModuleActions, Overflow, Theme};

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
    },
    /// A click on a bubble. UI only, never answered.
    BubbleClick { bubble: BubbleId },
    /// Answered with `status`.
    Status,
    /// Reloads config and theme. Answered with `ok` or `error`.
    Reload,
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
    /// The latest state a module published. Sent to the UI after `hello` for
    /// every module, then whenever it changes.
    State { module: String, state: Value },
    /// What the island shows now. `None` when no activity exists at all.
    Present { activity: Option<Activity> },
    /// Every bubble, in drawing order. Sent to the UI after `hello` and on
    /// every change.
    Bubbles {
        bubbles: Vec<Bubble>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        overflow: Vec<Overflow>,
    },
    /// Design tokens. Sent to the UI after `hello` and on reload.
    Theme { theme: Theme },
    /// A request succeeded.
    Ok,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{ActionSpec, Area, ArgSpec, decode, encode};

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
        });
        round_trip_client(ClientMessage::BubbleClick {
            bubble: BubbleId(3),
        });
        round_trip_client(ClientMessage::Status);
        round_trip_client(ClientMessage::Reload);
        round_trip_client(ClientMessage::ListActions { module: None });
        round_trip_client(ClientMessage::ListActions {
            module: Some("osd".into()),
        });
    }

    #[test]
    fn every_daemon_message_round_trips() {
        round_trip_daemon(DaemonMessage::Hello {
            api: 1,
            version: "0.1.0".into(),
        });
        round_trip_daemon(DaemonMessage::Modules {
            modules: vec!["idle".into()],
        });
        round_trip_daemon(DaemonMessage::State {
            module: "idle".into(),
            state: json!({ "unread": 2 }),
        });
        round_trip_daemon(DaemonMessage::Present {
            activity: Some(activity()),
        });
        round_trip_daemon(DaemonMessage::Present { activity: None });
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
            }],
            overflow: vec![Overflow {
                area: Area::Right,
                hidden: 2,
            }],
        });
        round_trip_daemon(DaemonMessage::Theme {
            theme: Theme::default(),
        });
        round_trip_daemon(DaemonMessage::Ok);
        round_trip_daemon(DaemonMessage::Status {
            status: Status {
                version: "0.1.0".into(),
                api: 1,
                ui_connected: true,
                modules: vec!["idle".into()],
                compositor: CompositorStatus {
                    backend: "wayland".into(),
                    outputs: vec!["DP-3".into()],
                    workspaces: 4,
                    focused: Some("DP-3".into()),
                },
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
                kind: EventKind::HoverEnter
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

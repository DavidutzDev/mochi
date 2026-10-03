//! Messages exchanged over the socket, one JSON object per line.

use serde::{Deserialize, Serialize};

pub const API: u32 = 1;

/// Sent to the daemon by Quickshell or the CLI.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Incoming {
    Hello {
        api: u32,
        role: Role,
    },
    Event {
        activity: u64,
        kind: EventKind,
    },
    Command {
        module: String,
        action: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

/// Sent by the daemon.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outgoing {
    Hello { api: u32 },
    Present { activity: Activity },
    Ok,
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Ui,
    Ctl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Click,
    HoverEnter,
    HoverLeave,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub id: u64,
    pub module: String,
    pub view: String,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ui_messages() {
        let hello: Incoming =
            serde_json::from_str(r#"{"type":"hello","api":1,"role":"ui"}"#).unwrap();
        assert!(matches!(
            hello,
            Incoming::Hello {
                api: 1,
                role: Role::Ui
            }
        ));

        let event: Incoming =
            serde_json::from_str(r#"{"type":"event","activity":3,"kind":"hover_enter"}"#).unwrap();
        assert!(matches!(
            event,
            Incoming::Event {
                activity: 3,
                kind: EventKind::HoverEnter
            }
        ));
    }

    #[test]
    fn command_args_default_to_empty() {
        let command: Incoming =
            serde_json::from_str(r#"{"type":"command","module":"idle","action":"show"}"#).unwrap();
        assert!(matches!(command, Incoming::Command { args, .. } if args.is_empty()));
    }

    #[test]
    fn serializes_present() {
        let message = Outgoing::Present {
            activity: Activity {
                id: 1,
                module: "idle".into(),
                view: "Pill".into(),
                payload: serde_json::json!({}),
            },
        };
        assert_eq!(
            serde_json::to_string(&message).unwrap(),
            r#"{"type":"present","activity":{"id":1,"module":"idle","view":"Pill","payload":{}}}"#
        );
    }
}

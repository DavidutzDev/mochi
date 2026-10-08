//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "popup", "Expanded", "Notifications")
            .icon("notifications")
            .order(10)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.1",
                "caption": "Notifications show on the island. The ones you miss wait behind a bubble, and Do Not Disturb holds them all.",
                "payload": {
                    "id": 12,
                    "app": "Discord",
                    "icon": "discord",
                    "image": null,
                    "summary": "Alex",
                    "body": "Are we still on for tonight?",
                    "line": "Are we still on for tonight?",
                    "actions": [
                        {
                            "key": "default",
                            "label": "View"
                        }
                    ],
                    "default": true,
                    "reply": null,
                    "urgency": "normal",
                    "received_ms": 1_760_000_000_000_i64
                }
            })),
        ContributionSpec::new("tour", "step", "reply", "Expanded", "Replies")
            .icon("notifications")
            .order(11)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.7",
                "caption": "Messages keep their bold, italics and links, and apps that take a reply get a Reply button.",
                "payload": {
                    "id": 12,
                    "app": "Discord",
                    "icon": "discord",
                    "image": null,
                    "summary": "Alex",
                    "body": "<b>Ship it</b> tonight, the notes are <a href=\"https://example.com\">here</a>.",
                    "line": "Ship it tonight, the notes are here.",
                    "actions": [
                        {
                            "key": "default",
                            "label": "View"
                        }
                    ],
                    "default": true,
                    "reply": "Reply",
                    "urgency": "normal",
                    "received_ms": 1_760_000_000_000_i64
                }
            })),
        ContributionSpec::new("tour", "step", "missed", "Count", "Missed notifications")
            .icon("notifications")
            .order(12)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.1",
                "place": "bubble",
                "caption": "Missed notifications wait in a bubble; a click opens them.",
                "payload": {
                    "area": "center-right",
                    "count": 3
                }
            })),
    ]
}

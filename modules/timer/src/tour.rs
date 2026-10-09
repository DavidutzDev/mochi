//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "notice", "Notice", "Focus timer")
            .icon("timer")
            .order(47)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.10",
                "caption": "A focus timer counts down in a bubble by the island, and says when to take a break. Start it from its card in the control center, or with mochi ipc timer start.",
                "payload": {
                    "finished": "focus",
                    "next": "break",
                    "minutes": 5,
                    "long": false,
                    "started": false,
                    "sessions": 2
                }
            })),
    ]
}

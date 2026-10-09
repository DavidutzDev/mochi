//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "clock", "Pill", "The clock")
            .icon("schedule")
            .order(10)
            .options(json!({
                "chapter": "island",
                "since": "0.0.1",
                "caption": "When nothing else is on, the island is a clock. A click opens the control center.",
                "payload": {
                    "format": "HH:mm"
                }
            })),
        ContributionSpec::new("tour", "step", "hover", "Pill", "Rest on the clock")
            .icon("schedule")
            .order(11)
            .options(json!({
                "chapter": "island",
                "since": "0.0.7",
                "caption": "Rest the pointer on the clock to run an action of your choice, like showing your workspaces: hover in its settings.",
                "payload": {
                    "format": "HH:mm"
                }
            })),
    ]
}

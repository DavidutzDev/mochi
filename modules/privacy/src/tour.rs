//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "dot", "Wide", "Privacy")
            .icon("mic")
            .order(45)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.8",
                "caption": "While an app listens to the microphone or watches through the camera, a dot stays next to the island. A click on it mutes the microphone.",
                "payload": {
                    "microphone": ["Discord"],
                    "camera": ["firefox"],
                    "muted": false
                }
            })),
    ]
}

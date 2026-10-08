//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "volume", "Volume", "Volume")
            .icon("tune")
            .order(20)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.1",
                "caption": "Volume, mute, a new output, Caps Lock: changes show as they happen, whatever made them. Scrolling on the volume changes it.",
                "payload": {
                    "percent": 45,
                    "muted": false
                }
            })),
    ]
}

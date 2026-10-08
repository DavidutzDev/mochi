//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "notice", "Notice", "Battery")
            .icon("battery_full")
            .order(40)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.5",
                "caption": "On a laptop, the battery speaks up at the levels you pick, and a bubble stays while it's low.",
                "payload": {
                    "text": "Battery at 20%",
                    "level": 20,
                    "charging": false,
                    "critical": false
                }
            })),
    ]
}

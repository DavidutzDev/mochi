//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "toggle", "Card", "Night light")
            .icon("nightlight")
            .order(55)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.8",
                "caption": "Night light warms the screens by hand, between two times, or from sunset to sunrise, with no other program.",
                "payload": {
                    "on": true,
                    "temperature": 4000,
                    "current": 4000,
                    "schedule": "sun",
                    "forced": false,
                    "available": true,
                    "problem": null
                }
            })),
    ]
}

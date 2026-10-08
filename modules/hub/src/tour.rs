//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "home", "Hub", "The hub")
            .icon("space_dashboard")
            .order(10)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.1",
                "caption": "The hub: a card for each thing worth a glance, and a page per module below. Bind mochi ipc hub toggle to a key.",
                "payload": {
                    "page": "home",
                    "width": 860,
                    "height": 420
                }
            })),
    ]
}

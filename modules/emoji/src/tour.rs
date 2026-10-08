//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "picker", "Picker", "Emoji")
            .icon("mood")
            .order(50)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.6",
                "caption": "Every emoji in a grid, searchable, with your recent ones first. Holding one picks its skin tone.",
                "payload": {}
            })),
    ]
}

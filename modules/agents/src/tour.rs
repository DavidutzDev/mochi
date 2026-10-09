//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "waiting", "Notice", "Coding agents")
            .icon("smart_toy")
            .order(46)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.10",
                "caption": "Claude Code and T3 Code say on the island when they need you or finish, and a bubble keeps a mark for each session.",
                "payload": {
                    "id": "tour",
                    "app": "Claude Code",
                    "title": "mochi-shell",
                    "state": "waiting",
                    "since": 0
                }
            })),
    ]
}

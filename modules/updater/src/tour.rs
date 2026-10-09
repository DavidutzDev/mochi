//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "notice", "Notice", "Updates")
            .icon("system_update")
            .order(14)
            .options(json!({
                "chapter": "settings",
                "since": "0.0.9",
                "caption": "When a new release is out, the island says so once. Its page in the settings has the changelog, and the update the way you installed Mochi.",
                "payload": {
                    "version": "0.1.0",
                    "current": "0.0.9"
                }
            })),
    ]
}

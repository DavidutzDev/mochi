//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "drawer", "Drawer", "Tray")
            .icon("apps")
            .order(30)
            .options(json!({
                "chapter": "island",
                "since": "0.0.5",
                "place": "bubble",
                "caption": "Apps' tray icons wait in a drawer bubble, with their menus. Pin the ones you use to give them a bubble of their own.",
                "payload": {
                    "area": "right",
                    "count": 3,
                    "attention": false
                }
            })),
    ]
}

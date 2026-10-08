//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "clock", "Clock", "Widgets")
            .icon("widgets")
            .order(10)
            .options(json!({
                "chapter": "desktop",
                "since": "0.0.6",
                "place": "widget",
                "size": [
                    224,
                    112
                ],
                "caption": "Widgets put views from any module on your desktop, under the windows. mochi ipc widgets edit arranges them.",
                "payload": {
                    "zones": {}
                },
                "properties": {
                    "settings": {
                        "timezone": "",
                        "hours": "24",
                        "seconds": true,
                        "date": true
                    },
                    "instance": "tour"
                }
            })),
    ]
}

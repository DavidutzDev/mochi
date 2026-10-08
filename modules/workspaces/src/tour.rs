//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "switch", "Workspaces", "Workspaces")
            .icon("view_carousel")
            .order(20)
            .options(json!({
                "chapter": "island",
                "since": "0.0.1",
                "caption": "Switching workspaces shows where you are, on the monitor that changed. mochi ipc workspaces show keeps the dots up to click.",
                "payload": {
                    "output": "",
                    "label": "",
                    "reason": "switch",
                    "active": "2",
                    "urgent": "4",
                    "workspaces": [
                        {
                            "name": "1",
                            "active": false,
                            "urgent": false
                        },
                        {
                            "name": "2",
                            "active": true,
                            "urgent": false
                        },
                        {
                            "name": "3",
                            "active": false,
                            "urgent": false
                        },
                        {
                            "name": "4",
                            "active": false,
                            "urgent": true
                        },
                        {
                            "name": "5",
                            "active": false,
                            "urgent": false
                        }
                    ]
                }
            })),
    ]
}

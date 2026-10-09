//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "page", "Page", "Power")
            .icon("power_settings_new")
            .order(60)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.1",
                "place": "card",
                "size": [
                    640,
                    260
                ],
                "caption": "Lock, log out, suspend, reboot and shut down, and your power profile, on the control center's Power page.",
                "payload": {
                    "buttons": [
                        {
                            "action": "lock",
                            "label": "Lock",
                            "icon": "lock",
                            "confirm": false
                        },
                        {
                            "action": "logout",
                            "label": "Log out",
                            "icon": "logout",
                            "confirm": true
                        },
                        {
                            "action": "suspend",
                            "label": "Suspend",
                            "icon": "moon",
                            "confirm": false
                        },
                        {
                            "action": "reboot",
                            "label": "Reboot",
                            "icon": "reboot",
                            "confirm": true
                        },
                        {
                            "action": "poweroff",
                            "label": "Shut down",
                            "icon": "power",
                            "confirm": true
                        }
                    ],
                    "profiles": [
                        "power-saver",
                        "balanced",
                        "performance"
                    ],
                    "profile": "balanced"
                }
            })),
    ]
}

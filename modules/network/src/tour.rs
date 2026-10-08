//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "card", "Card", "Network")
            .icon("wifi")
            .order(70)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.5",
                "place": "card",
                "size": [
                    280,
                    96
                ],
                "caption": "Wi-Fi, Ethernet, VPNs and airplane mode from NetworkManager, with a page in the hub to connect.",
                "payload": {
                    "available": true,
                    "status": {
                        "icon": "wifi",
                        "label": "HomeNet",
                        "vpn": false
                    },
                    "wifi": {
                        "available": true,
                        "enabled": true,
                        "hardware": true
                    },
                    "airplane": false,
                    "networks": [
                        {
                            "ssid": "HomeNet",
                            "strength": 82,
                            "icon": "wifi",
                            "secure": true,
                            "enterprise": false,
                            "saved": true,
                            "connected": true
                        },
                        {
                            "ssid": "Café",
                            "strength": 40,
                            "icon": "wifi-2",
                            "secure": false,
                            "enterprise": false,
                            "saved": false,
                            "connected": false
                        }
                    ],
                    "wired": [],
                    "vpns": []
                }
            })),
    ]
}

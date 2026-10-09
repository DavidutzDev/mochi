//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "card", "Card", "Bluetooth")
            .icon("bluetooth")
            .order(71)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.5",
                "place": "card",
                "size": [
                    280,
                    96
                ],
                "caption": "Bluetooth devices with their battery, and pairing, from the control center.",
                "payload": {
                    "available": true,
                    "powered": true,
                    "scanning": false,
                    "paired": [
                        {
                            "name": "WH-1000XM4",
                            "address": "AA:BB:CC:DD:EE:FF",
                            "icon": "audio-headphones",
                            "paired": true,
                            "connected": true,
                            "battery": 72
                        }
                    ],
                    "found": [],
                    "connected": [
                        "WH-1000XM4"
                    ]
                }
            })),
    ]
}

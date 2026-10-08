//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "page", "Page", "Performance")
            .icon("monitor_heart")
            .order(80)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.5",
                "place": "card",
                "size": [
                    760,
                    420
                ],
                "caption": "CPU, memory, GPU, disks and network, with the busiest processes; yours can be ended from here.",
                "payload": {
                    "cpu": {
                        "usage": 23,
                        "cores": 16,
                        "temperature": 58,
                        "history": [
                            10,
                            15,
                            23,
                            30,
                            18,
                            22,
                            25,
                            23
                        ]
                    },
                    "memory": {
                        "percent": 41,
                        "used": "12.8 GB",
                        "total": "31.2 GB",
                        "swap_used": "0 B",
                        "swap_total": "8.0 GB",
                        "history": [
                            40,
                            41,
                            41,
                            42,
                            41
                        ]
                    },
                    "gpu": {
                        "name": "RTX 3070",
                        "usage": 12,
                        "memory_used": 1200,
                        "memory_total": 8192,
                        "temperature": 49,
                        "history": [
                            10,
                            12,
                            9,
                            14,
                            12
                        ]
                    },
                    "disk": {
                        "in": "1.2 MB/s",
                        "out": "0 B/s",
                        "in_history": [
                            0,
                            1,
                            3,
                            1,
                            0
                        ],
                        "out_history": [
                            0,
                            0,
                            1,
                            0,
                            0
                        ]
                    },
                    "network": {
                        "in": "340 KB/s",
                        "out": "20 KB/s",
                        "in_history": [
                            1,
                            3,
                            2,
                            4,
                            3
                        ],
                        "out_history": [
                            0,
                            1,
                            0,
                            1,
                            0
                        ]
                    },
                    "sort": "cpu",
                    "processes": [
                        {
                            "pid": 4242,
                            "name": "firefox",
                            "cpu": 12.5,
                            "memory": "1.4 GB",
                            "disk": "0 B/s",
                            "own": true
                        },
                        {
                            "pid": 2121,
                            "name": "spotify",
                            "cpu": 3.1,
                            "memory": "420 MB",
                            "disk": "0 B/s",
                            "own": true
                        }
                    ],
                    "critical": []
                }
            })),
    ]
}

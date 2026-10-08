//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "history", "Picker", "Clipboard")
            .icon("content_paste")
            .order(30)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.4",
                "caption": "Everything you copy, searchable. Enter pastes it into the window you were in.",
                "payload": {
                    "query": "",
                    "paused": false,
                    "now": 1760000060,
                    "results": [
                        {
                            "id": 3,
                            "kind": "text",
                            "pinned": true,
                            "text": "ssh deploy@mochi.example",
                            "lines": 1,
                            "size": "24 B",
                            "time": 1760000000,
                            "width": null,
                            "height": null,
                            "image": null
                        },
                        {
                            "id": 2,
                            "kind": "text",
                            "pinned": false,
                            "text": "https://github.com/DavidutzDev/mochi",
                            "lines": 1,
                            "size": "36 B",
                            "time": 1760000000,
                            "width": null,
                            "height": null,
                            "image": null
                        },
                        {
                            "id": 1,
                            "kind": "text",
                            "pinned": false,
                            "text": "Meeting moved to 3 pm, same room.",
                            "lines": 1,
                            "size": "33 B",
                            "time": 1760000000,
                            "width": null,
                            "height": null,
                            "image": null
                        }
                    ],
                    "detail": {
                        "id": 3,
                        "text": "ssh deploy@mochi.example"
                    }
                }
            })),
        ContributionSpec::new("tour", "step", "pins", "Picker", "Pins")
            .icon("push_pin")
            .order(31)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.7",
                "caption": "Pin what you paste often: pins stay on top, and clearing the history leaves them. Ctrl+P pins the selected entry.",
                "payload": {
                    "query": "",
                    "paused": false,
                    "now": 1760000060,
                    "results": [
                        {
                            "id": 3,
                            "kind": "text",
                            "pinned": true,
                            "text": "ssh deploy@mochi.example",
                            "lines": 1,
                            "size": "24 B",
                            "time": 1760000000,
                            "width": null,
                            "height": null,
                            "image": null
                        },
                        {
                            "id": 2,
                            "kind": "text",
                            "pinned": false,
                            "text": "https://github.com/DavidutzDev/mochi",
                            "lines": 1,
                            "size": "36 B",
                            "time": 1760000000,
                            "width": null,
                            "height": null,
                            "image": null
                        },
                        {
                            "id": 1,
                            "kind": "text",
                            "pinned": false,
                            "text": "Meeting moved to 3 pm, same room.",
                            "lines": 1,
                            "size": "33 B",
                            "time": 1760000000,
                            "width": null,
                            "height": null,
                            "image": null
                        }
                    ],
                    "detail": {
                        "id": 3,
                        "text": "ssh deploy@mochi.example"
                    }
                }
            })),
    ]
}

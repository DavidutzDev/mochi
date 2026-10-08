//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "todo", "Todo", "To-do lists")
            .icon("checklist")
            .order(20)
            .options(json!({
                "chapter": "desktop",
                "since": "0.0.6",
                "place": "widget",
                "size": [
                    256,
                    224
                ],
                "caption": "To-do lists and notes, as widgets, each with pages of its own.",
                "payload": {
                    "pages": {
                        "tour": {
                            "items": [
                                {
                                    "text": "Take the tour",
                                    "done": true
                                },
                                {
                                    "text": "Pick an accent color",
                                    "done": false
                                },
                                {
                                    "text": "Bind the launcher to a key",
                                    "done": false
                                }
                            ]
                        }
                    }
                },
                "properties": {
                    "settings": {
                        "title": "To-do"
                    },
                    "instance": "tour"
                }
            })),
    ]
}

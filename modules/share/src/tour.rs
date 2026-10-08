//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "picker", "Picker", "Screen sharing")
            .icon("screen_share")
            .order(20)
            .options(json!({
                "chapter": "capture",
                "since": "0.0.4",
                "caption": "When an app asks to share your screen, pick a screen, a window or an area. A switchable share changes source without the app asking again.",
                "payload": {
                    "remember": false,
                    "switchable": true,
                    "framerate": 60,
                    "resolution": "native",
                    "switching": false,
                    "windows": [
                        {
                            "handle": "94",
                            "class": "firefox",
                            "title": "Docs",
                            "address": ""
                        },
                        {
                            "handle": "95",
                            "class": "kitty",
                            "title": "Terminal",
                            "address": ""
                        }
                    ],
                    "drawing": false
                }
            })),
    ]
}

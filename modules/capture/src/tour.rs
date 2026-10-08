//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "screenshot", "Preview", "Screenshots")
            .icon("screenshot_region")
            .order(10)
            .options(json!({
                "chapter": "capture",
                "since": "0.0.2",
                "caption": "mochi ipc capture screenshot picks a region, a window or a screen. The shot lands here, copied, with Edit and Open.",
                "payload": {
                    "kind": "screenshot",
                    "title": "Screenshot",
                    "path": "",
                    "name": "Screenshot from 2026-10-08.png",
                    "folder": "~/Pictures/Screenshots",
                    "copied": true,
                    "editable": true
                }
            })),
        ContributionSpec::new("tour", "step", "recording", "Recording", "Recording")
            .icon("screenshot_region")
            .order(11)
            .options(json!({
                "chapter": "capture",
                "since": "0.0.2",
                "place": "bubble",
                "caption": "A screen recording shows as a bubble; a click stops it and it lands in the same place.",
                "payload": {
                    "area": "center-right"
                }
            })),
    ]
}

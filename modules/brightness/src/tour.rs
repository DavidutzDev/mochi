//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "level", "Osd", "Brightness")
            .icon("light_mode")
            .order(22)
            .options(json!({
                "chapter": "notices",
                "since": "0.0.8",
                "caption": "Brightness, for the laptop's screen and for monitors over DDC/CI. The control center has a slider for each, and scrolling here changes it.",
                "payload": {
                    "display": "backlight",
                    "name": "Built-in",
                    "icon": "light_mode",
                    "percent": 60
                }
            })),
    ]
}

//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "picked", "Picked", "Colors")
            .icon("colorize")
            .order(30)
            .options(json!({
                "chapter": "capture",
                "since": "0.0.6",
                "caption": "mochi ipc colors pick freezes the screen under a magnifier; a click copies the exact pixel as HEX, RGB, HSL or OKLCH.",
                "payload": {
                    "color": "#7aa2f7",
                    "swatch": "#7aa2f7",
                    "text": "#7AA2F7",
                    "formats": [
                        {
                            "format": "hex",
                            "label": "HEX",
                            "text": "#7AA2F7"
                        },
                        {
                            "format": "rgb",
                            "label": "RGB",
                            "text": "rgb(122, 162, 247)"
                        },
                        {
                            "format": "hsl",
                            "label": "HSL",
                            "text": "hsl(221, 89%, 72%)"
                        },
                        {
                            "format": "oklch",
                            "label": "OKLCH",
                            "text": "oklch(0.72 0.13 263)"
                        }
                    ]
                }
            })),
        ContributionSpec::new("tour", "step", "chooser", "Page", "Color chooser")
            .icon("colorize")
            .order(31)
            .options(json!({
                "chapter": "capture",
                "since": "0.0.7",
                "place": "card",
                "size": [
                    760,
                    360
                ],
                "caption": "The control center's Colors page is a color chooser, with the colors you picked beside it.",
                "payload": {
                    "format": "hex",
                    "picking": false,
                    "history": [
                        {
                            "color": "#7aa2f7",
                            "swatch": "#7aa2f7",
                            "text": "#7AA2F7",
                            "formats": [
                                {
                                    "format": "hex",
                                    "label": "HEX",
                                    "text": "#7AA2F7"
                                }
                            ]
                        },
                        {
                            "color": "#f7768e",
                            "swatch": "#f7768e",
                            "text": "#F7768E",
                            "formats": [
                                {
                                    "format": "hex",
                                    "label": "HEX",
                                    "text": "#F7768E"
                                }
                            ]
                        },
                        {
                            "color": "#9ece6a",
                            "swatch": "#9ece6a",
                            "text": "#9ECE6A",
                            "formats": [
                                {
                                    "format": "hex",
                                    "label": "HEX",
                                    "text": "#9ECE6A"
                                }
                            ]
                        }
                    ]
                }
            })),
    ]
}

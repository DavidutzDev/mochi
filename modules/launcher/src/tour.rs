//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "apps", "Launcher", "The launcher")
            .icon("search")
            .order(20)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.1",
                "caption": "The launcher finds your apps as you type, the ones you open most first.",
                "payload": {
                    "query": "fi",
                    "results": [
                        {
                            "key": "1",
                            "title": "Firefox",
                            "subtitle": "Web Browser",
                            "icon": "firefox",
                            "glyph": null,
                            "color": null,
                            "small": false,
                            "section": "Apps"
                        },
                        {
                            "key": "2",
                            "title": "Files",
                            "subtitle": "Access and organize files",
                            "icon": "org.gnome.Nautilus",
                            "glyph": null,
                            "color": null,
                            "small": false,
                            "section": "Apps"
                        },
                        {
                            "key": "3",
                            "title": "Firewall",
                            "subtitle": "Configure the firewall",
                            "icon": "firewall-config",
                            "glyph": null,
                            "color": null,
                            "small": false,
                            "section": "Apps"
                        }
                    ],
                    "sections": false,
                    "searching": false
                }
            })),
        ContributionSpec::new("tour", "step", "providers", "Launcher", "Prefixes")
            .icon("search")
            .order(21)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.6",
                "caption": "A prefix asks one thing: = for math, / for files, > for commands, : for emoji, # for colors, and !w, !g or !gh for the web.",
                "payload": {
                    "query": "=12*7",
                    "results": [
                        {
                            "key": "1",
                            "title": "84",
                            "subtitle": "12 × 7",
                            "icon": null,
                            "glyph": "=",
                            "color": null,
                            "small": false,
                            "section": "Calculator"
                        },
                        {
                            "key": "2",
                            "title": "#7aa2f7",
                            "subtitle": "rgb(122, 162, 247)",
                            "icon": null,
                            "glyph": null,
                            "color": "#7aa2f7",
                            "small": false,
                            "section": "Colors"
                        },
                        {
                            "key": "3",
                            "title": "Search DuckDuckGo for mochi",
                            "subtitle": "!w",
                            "icon": "web",
                            "glyph": null,
                            "color": null,
                            "small": false,
                            "section": "Web"
                        }
                    ],
                    "sections": true,
                    "searching": false
                }
            })),
    ]
}

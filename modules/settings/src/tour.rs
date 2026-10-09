//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "panel", "Panel", "Settings")
            .icon("settings")
            .order(10)
            .options(json!({
                "chapter": "settings",
                "since": "0.0.7",
                "caption": "Settings: every option of Mochi, applied as you change it. Copy gives them back as Nix or TOML for your files.",
                "payload": {
                    "section": "colors",
                    "option": ""
                }
            })),
        ContributionSpec::new("tour", "step", "preview", "Panel", "Preview")
            .icon("visibility")
            .order(11)
            .options(json!({
                "chapter": "settings",
                "since": "0.0.8",
                "caption": "Preview tries changes without keeping them: they apply at once, and Keep or Drop decides. A reload drops them too.",
                "payload": {
                    "section": "layout",
                    "option": ""
                }
            })),
        ContributionSpec::new("tour", "step", "bento", "Panel", "Bento")
            .icon("storefront")
            .order(12)
            .options(json!({
                "chapter": "settings",
                "since": "0.0.8",
                "caption": "Bento: themes, plugins and whole setups other people share, and yours to share with them. It stays off until you turn it on, and your own setup is always one switch away.",
                "payload": {
                    "section": "bento",
                    "option": ""
                }
            })),
    ]
}

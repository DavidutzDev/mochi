//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "actions", "Actions", "Drop files")
            .icon("place_item")
            .order(60)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.8",
                "caption": "Drop files on the island: it says what came and offers what fits, like merging PDFs, converting images or extracting an archive.",
                "payload": {
                    "summary": "3 PDFs",
                    "count": 3,
                    "actions": [
                        { "id": "zip", "label": "Compress", "icon": "folder_zip" },
                        { "id": "merge", "label": "Merge PDFs", "icon": "picture_as_pdf" },
                        { "id": "copy", "label": "Copy paths", "icon": "content_copy" },
                        { "id": "open", "label": "Open", "icon": "open_in_new" }
                    ],
                    "running": null,
                    "message": null,
                    "failed": false,
                    "result": false
                }
            })),
    ]
}

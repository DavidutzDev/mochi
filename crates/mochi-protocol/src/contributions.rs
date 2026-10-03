//! What modules offer each other: a page for the hub, a card on its home
//! screen. A module declares them up front; the module they're meant for
//! finds them, and they're simply unused when it isn't enabled.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One thing a module offers another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    /// The module offering it, whose view it is.
    pub module: String,
    /// The module it's for, like `hub`.
    pub target: String,
    /// What it is to the target, like `page` or `card`. The target decides
    /// which kinds it takes.
    pub kind: String,
    /// Unique within the offering module.
    pub id: String,
    /// `modules/<module>/<view>.qml`. It gets the offering module's
    /// published state as `payload`.
    pub view: String,
    pub title: String,
    /// An icon name from the icon theme, or a path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Lower comes first.
    #[serde(default)]
    pub order: i32,
    /// Anything else the target reads, like a card's width.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub options: Value,
}

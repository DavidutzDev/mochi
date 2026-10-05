//! Bubbles: small, long-lived status items in the five areas along the
//! island's edge, next to the island or at the screen's sides.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One of the five places along the edge, from left to right. The island
/// sits in one of them too.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Area {
    /// Against the left side of the screen.
    Left,
    /// Just left of whatever is in the center.
    CenterLeft,
    #[default]
    Center,
    /// Just right of whatever is in the center.
    CenterRight,
    /// Against the right side of the screen.
    Right,
}

impl Area {
    pub const ALL: [Self; 5] = [
        Self::Left,
        Self::CenterLeft,
        Self::Center,
        Self::CenterRight,
        Self::Right,
    ];
}

/// Identifies one bubble for its whole life, across the daemon and the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BubbleId(pub u64);

impl fmt::Display for BubbleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A bubble as the UI draws it. Snapshots list bubbles in drawing order:
/// area by area, each from left to right. Consecutive bubbles with the same
/// `group` share one pill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bubble {
    pub id: BubbleId,
    pub module: String,
    /// The module's replacement key. A bubble with the same module and key
    /// continues the previous one, so the UI keeps its view and updates the
    /// payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The view to load: `modules/<module>/<view>.qml`.
    pub view: String,
    /// Whether `view` is a wide view with text, drawn in a pill. Otherwise it
    /// is a small one, drawn in a round bubble.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wide: bool,
    pub payload: Value,
    pub area: Area,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Higher is more important: the front of a stack.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub priority: u8,
    /// Goes up whenever the bubble has news, like when it appears or its
    /// module says something new happened; payload updates alone leave it.
    /// A stack brings a bubble whose news went up to the front for a while.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub news: u64,
}

fn is_zero<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Bubbles stacking, one per area, from `[bubbles] stack`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stacking {
    /// How long a bubble with news stays in front, in milliseconds.
    pub news_ms: u64,
}

/// How many bubbles an area has beyond its maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overflow {
    pub area: Area,
    pub hidden: u32,
}

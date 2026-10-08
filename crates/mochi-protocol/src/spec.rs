//! What modules hand the daemon: activities for the island, bubbles, and
//! the parsed arguments of their actions. Builtin modules use these types
//! directly; plugin backends send them as JSON, so they serialize too.

#![warn(missing_docs)]

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Area;

/// Higher values win. The named levels leave room for anything in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Priority(pub u8);

impl Priority {
    /// The idle island. Everything else interrupts it.
    pub const IDLE: Self = Self(0);
    /// Background news, like a finished download.
    pub const LOW: Self = Self(25);
    /// The default.
    pub const NORMAL: Self = Self(50);
    /// Worth interrupting for, like a call or a low battery.
    pub const HIGH: Self = Self(75);
    /// Needs attention now, like a critical battery.
    pub const URGENT: Self = Self(100);
    /// Interrupts anything, even uninterruptible activities, and nothing
    /// interrupts it. For capturing the screen as it is: a screenshot of the
    /// open launcher has to open over it.
    pub const TOP: Self = Self(u8::MAX);
}

/// What a new activity does when the shown one has the same priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamePriority {
    /// Wait until the shown activity ends.
    #[default]
    Queue,
    /// Interrupt it. It comes back afterwards.
    Stack,
}

/// Everything a module says about an activity it wants to show.
///
/// As JSON, only `compact` is required, and durations are milliseconds:
/// `{"compact": "Timer", "timeout_ms": 4000}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActivitySpec {
    /// Replaces the module's existing activity with the same key.
    pub key: Option<String>,
    /// View shown by default: `modules/<module>/<compact>.qml`.
    pub compact: String,
    /// View shown after a click. Without one, clicks go to the module.
    pub expanded: Option<String>,
    /// Opens on the expanded view for this long, then collapses.
    #[serde(
        rename = "expand_for_ms",
        with = "millis",
        skip_serializing_if = "Option::is_none"
    )]
    pub expand_for: Option<Duration>,
    /// What the views get as `payload`.
    pub payload: Value,
    /// Who shows first, and who interrupts whom.
    pub priority: Priority,
    /// Time on screen before the activity ends. `None` never times out.
    #[serde(
        rename = "timeout_ms",
        with = "millis",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeout: Option<Duration>,
    /// Whether a higher priority may interrupt it. [`Priority::TOP`]
    /// interrupts anything.
    pub interruptible: bool,
    /// What it does when the shown activity has the same priority.
    pub same_priority: SamePriority,
    /// Takes the keyboard while shown; a click outside the island dismisses
    /// it.
    pub modal: bool,
    /// A full-screen view under the island on every monitor.
    pub overlay: Option<String>,
    /// Never closes on a click outside, for feedback like the volume OSD
    /// that shows while the user is busy elsewhere.
    pub passive: bool,
    /// Shown at once or not at all, never queued or suspended: feedback
    /// like the volume OSD is stale by the time it would come back.
    pub fleeting: bool,
    /// The monitor a modal activity shows on. The daemon picks one from
    /// `[island] panels` when the module leaves it out.
    pub output: Option<String>,
}

impl Default for ActivitySpec {
    fn default() -> Self {
        Self::new("")
    }
}

impl ActivitySpec {
    /// A normal-priority, interruptible activity with no timeout.
    pub fn new(compact: impl Into<String>) -> Self {
        Self {
            key: None,
            compact: compact.into(),
            expanded: None,
            expand_for: None,
            payload: Value::Null,
            priority: Priority::NORMAL,
            timeout: None,
            interruptible: true,
            same_priority: SamePriority::Queue,
            modal: false,
            overlay: None,
            output: None,
            passive: false,
            fleeting: false,
        }
    }

    /// Replaces the module's activity with the same key, wherever it is,
    /// instead of adding one. With the same view too, the view updates in
    /// place.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// The view a click opens. Without one, clicks go to the module as
    /// `Clicked`.
    pub fn expanded(mut self, view: impl Into<String>) -> Self {
        self.expanded = Some(view.into());
        self
    }

    /// Opens on the expanded view, which needs one, and collapses after
    /// `duration` on screen. A keyed replacement that sets it again opens it
    /// again.
    pub fn expand_for(mut self, duration: Duration) -> Self {
        self.expand_for = Some(duration);
        self
    }

    /// What the views get as `payload`.
    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    /// See [`Priority`]; [`Priority::NORMAL`] by default.
    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Ends it after this long on screen. The time only runs while it's
    /// shown, not hovered and not expanded.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Nothing below [`Priority::TOP`] interrupts it.
    pub fn uninterruptible(mut self) -> Self {
        self.interruptible = false;
        self
    }

    /// For views that take typing, like a launcher.
    pub fn modal(mut self) -> Self {
        self.modal = true;
        self
    }

    /// Draws `view` over every monitor, under the island, while the activity
    /// shows: `modules/<module>/<view>.qml`, which gets the payload and its
    /// `screen`. Makes the activity modal. The island waits to show the
    /// activity until the overlay's `ready` property is true, so an overlay
    /// can capture the screen before the island changes.
    pub fn overlay(mut self, view: impl Into<String>) -> Self {
        self.overlay = Some(view.into());
        self.modal = true;
        self
    }

    /// Never closes on a click outside: see [`ActivitySpec::passive`].
    pub fn passive(mut self) -> Self {
        self.passive = true;
        self
    }

    /// Never waits behind another activity: see [`ActivitySpec::fleeting`].
    pub fn fleeting(mut self) -> Self {
        self.fleeting = true;
        self
    }

    /// Shows the activity on this monitor's island only, whatever
    /// `[island] panels` or `notices` say. Each monitor has an island of
    /// its own, so it waits only behind what that one shows.
    pub fn output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());
        self
    }

    /// [`SamePriority::Queue`] by default.
    pub fn same_priority(mut self, rule: SamePriority) -> Self {
        self.same_priority = rule;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Why an activity ended.
pub enum EndReason {
    /// Its timeout ran out.
    Expired,
    /// The user closed it.
    Dismissed,
    /// Its module withdrew it.
    Withdrawn,
    /// A newer activity with the same key took its place.
    Replaced,
    /// The user clicked outside the island, without having opened it: less
    /// than a dismissal, more like its time running out early.
    Outside,
}

/// Everything a module says about a bubble it wants to show.
///
/// As JSON, only `view` is required.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BubbleSpec {
    /// Replaces the module's existing bubble with the same key, keeping its
    /// place.
    pub key: Option<String>,
    /// `modules/<module>/<view>.qml`: a small view that fits a round bubble,
    /// an icon or a cover.
    pub view: String,
    /// A wider view with text, shown in a pill when the user asks for it
    /// with `wide = true`.
    pub wide: Option<String>,
    /// What the views get as `payload`.
    pub payload: Value,
    /// Where it goes; the user's `[bubbles.<module>]` settings win.
    pub area: Area,
    /// Bubbles with the same group in the same area share one pill.
    pub group: Option<String>,
    /// Lower goes further left.
    pub order: i32,
    /// Breaks ties in `order`, decides who is left out when an area is
    /// full, and who is in front of a stack.
    pub priority: Priority,
    /// Showing it again over the one with the same key is news: a stack
    /// brings it to the front for a while. A new bubble always is.
    pub news: bool,
    /// What the pointer resting on it shows beside it. A view can say more
    /// with a `tooltip` property, worked out from its payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

impl Default for BubbleSpec {
    fn default() -> Self {
        Self::new("")
    }
}

impl BubbleSpec {
    /// A normal-priority bubble on its own, right of the center.
    pub fn new(view: impl Into<String>) -> Self {
        Self {
            key: None,
            view: view.into(),
            wide: None,
            payload: Value::Null,
            area: Area::CenterRight,
            group: None,
            order: 0,
            priority: Priority::NORMAL,
            news: false,
            tooltip: None,
        }
    }

    /// What the pointer resting on it shows beside it.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Replaces the module's bubble with the same key in place.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// The wider view with text, for users who set `wide = true`.
    pub fn wide(mut self, view: impl Into<String>) -> Self {
        self.wide = Some(view.into());
        self
    }

    /// What the views get as `payload`.
    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    /// [`Area::CenterRight`] by default.
    pub fn area(mut self, area: Area) -> Self {
        self.area = area;
        self
    }

    /// Bubbles with the same group next to each other share a pill.
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Lower goes further left.
    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Breaks ties in `order`, picks who stays when an area is full and
    /// who is in front of a stack.
    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Marks this showing as news: see [`BubbleSpec::news`].
    pub fn news(mut self) -> Self {
        self.news = true;
        self
    }
}

/// One parsed argument, of the kind its action declares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    /// A `string` or `choice` argument, or `rest`.
    String(String),
    /// An `int` argument.
    Int(i64),
    /// A `float` argument.
    Float(f64),
    /// A `bool` argument.
    Bool(bool),
}

/// Parsed arguments, looked up by name.
///
/// As JSON, an object from names to values: `{"minutes": 25}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(
    from = "serde_json::Map<String, Value>",
    into = "serde_json::Map<String, Value>"
)]
pub struct Args(Vec<(String, ArgValue)>);

impl Args {
    /// An argument by name, whatever its kind.
    pub fn get(&self, name: &str) -> Option<&ArgValue> {
        self.0
            .iter()
            .find(|(arg, _)| arg == name)
            .map(|(_, value)| value)
    }

    /// The value of a `string`, `choice` or `rest` argument.
    pub fn str(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            ArgValue::String(value) => Some(value),
            _ => None,
        }
    }

    /// The value of an `int` argument.
    pub fn int(&self, name: &str) -> Option<i64> {
        match self.get(name)? {
            ArgValue::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// The value of a `float` argument.
    pub fn float(&self, name: &str) -> Option<f64> {
        match self.get(name)? {
            ArgValue::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// The value of a `bool` argument.
    pub fn bool(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            ArgValue::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

/// Why calling another module's action failed.
///
/// As JSON: `{"kind": "not_enabled", "detail": "media"}`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum CallError {
    /// The module isn't enabled: the usual case for a soft dependency.
    #[error("module {0:?} is not enabled")]
    NotEnabled(String),
    #[error("a module cannot call itself")]
    /// A module called its own action.
    Itself,
    #[error("{module} has no action {action:?}")]
    /// That module has no such action.
    UnknownAction {
        /// The module called.
        module: String,
        /// The action it doesn't have.
        action: String,
    },
    #[error("{0}")]
    /// The arguments don't fit the action; the message has its usage.
    InvalidArgs(String),
    /// The module ran the action and reported a failure, or isn't running.
    #[error("{0}")]
    Failed(String),
}

impl FromIterator<(String, ArgValue)> for Args {
    fn from_iter<I: IntoIterator<Item = (String, ArgValue)>>(pairs: I) -> Self {
        Self(pairs.into_iter().collect())
    }
}

impl From<serde_json::Map<String, Value>> for Args {
    /// Values that aren't strings, numbers or booleans are left out.
    fn from(map: serde_json::Map<String, Value>) -> Self {
        map.into_iter()
            .filter_map(|(name, value)| Some((name, serde_json::from_value(value).ok()?)))
            .collect()
    }
}

impl From<Args> for serde_json::Map<String, Value> {
    fn from(args: Args) -> Self {
        args.0
            .into_iter()
            .map(|(name, value)| {
                let value = match value {
                    ArgValue::String(value) => Value::from(value),
                    ArgValue::Int(value) => Value::from(value),
                    ArgValue::Float(value) => Value::from(value),
                    ArgValue::Bool(value) => Value::from(value),
                };
                (name, value)
            })
            .collect()
    }
}

/// Durations as whole milliseconds.
mod millis {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &Option<Duration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(duration) => {
                serializer.serialize_some(&u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Duration>, D::Error> {
        Ok(Option::<u64>::deserialize(deserializer)?.map(Duration::from_millis))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_activity_needs_only_its_view() {
        let spec: ActivitySpec = serde_json::from_value(json!({ "compact": "Timer" })).unwrap();
        assert_eq!(spec, ActivitySpec::new("Timer"));
    }

    #[test]
    fn activities_round_trip_with_durations_in_milliseconds() {
        let spec = ActivitySpec::new("Timer")
            .expanded("Big")
            .timeout(Duration::from_millis(1500))
            .priority(Priority::HIGH)
            .same_priority(SamePriority::Stack)
            .uninterruptible();
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["timeout_ms"], 1500);
        assert_eq!(json["priority"], 75);
        assert_eq!(json["same_priority"], "stack");
        assert_eq!(serde_json::from_value::<ActivitySpec>(json).unwrap(), spec);
    }

    #[test]
    fn bubbles_round_trip() {
        let spec = BubbleSpec::new("Dot")
            .key("timer")
            .area(Area::Right)
            .priority(Priority::LOW)
            .news();
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["area"], "right");
        assert_eq!(serde_json::from_value::<BubbleSpec>(json).unwrap(), spec);
        let short: BubbleSpec = serde_json::from_value(json!({ "view": "Dot" })).unwrap();
        assert_eq!(short, BubbleSpec::new("Dot"));
    }

    #[test]
    fn args_are_an_object() {
        let args: Args = [
            ("minutes".to_owned(), ArgValue::Int(25)),
            ("label".to_owned(), ArgValue::String("Write".into())),
            ("ratio".to_owned(), ArgValue::Float(0.5)),
            ("loud".to_owned(), ArgValue::Bool(true)),
        ]
        .into_iter()
        .collect();
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(
            json,
            json!({ "minutes": 25, "label": "Write", "ratio": 0.5, "loud": true })
        );
        let back: Args = serde_json::from_value(json).unwrap();
        assert_eq!(back.int("minutes"), Some(25));
        assert_eq!(back.str("label"), Some("Write"));
        assert_eq!(back.float("ratio"), Some(0.5));
        assert_eq!(back.bool("loud"), Some(true));
    }
}

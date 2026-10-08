//! Design tokens the UI reads from its `Theme` singleton. Users override them
//! in `theme.toml`; anything they leave out keeps the default below.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Area;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub colors: Colors,
    pub layout: Layout,
    pub motion: Motion,
    pub text: Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
/// Color roles. The defaults are "Obsidian": a black island that blends into
/// the bezel, graphite layers on it, and one accent used sparingly.
pub struct Colors {
    /// The island and the hub panel.
    pub background: Color,
    /// Cards and tiles on the background.
    pub surface: Color,
    /// Controls, tracks and dividers: one step above `surface`.
    pub raised: Color,
    /// Hovered or pressed controls: one step above `raised`.
    pub highlight: Color,
    pub foreground: Color,
    /// Secondary text and icons.
    pub muted: Color,
    /// Active, selected or important things, and nothing else.
    pub accent: Color,
    /// Text and icons on `accent`.
    pub on_accent: Color,
    /// Destructive actions, like a shut down waiting for its second click.
    pub danger: Color,
    pub success: Color,
    /// The hairline around the island and the bubbles.
    pub border: Color,
    /// The shadow under them. Transparent turns it off.
    pub shadow: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            background: Color::fixed("#f5000000"),
            surface: Color::fixed("#1c1c1e"),
            raised: Color::fixed("#2c2c2e"),
            highlight: Color::fixed("#3a3a3c"),
            foreground: Color::fixed("#ffffff"),
            muted: Color::fixed("#8e8e93"),
            accent: Color::fixed("#ff9f0a"),
            on_accent: Color::fixed("#000000"),
            danger: Color::fixed("#ff453a"),
            success: Color::fixed("#30d158"),
            border: Color::fixed("#14ffffff"),
            shadow: Color::fixed("#59000000"),
        }
    }
}

/// Where the island sits and what shape it takes. Sizes in logical pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Layout {
    pub mode: Mode,
    /// The screen edge the island and the bubbles sit against.
    pub anchor: Anchor,
    /// The area the island sits in.
    pub island: Area,
    /// Gap between the screen edges and everything on them in island mode.
    /// Notch mode always touches the edge.
    #[serde(alias = "top_margin")]
    #[schemars(range(min = 0, max = 48))]
    pub margin: u32,
    /// Gap between the island and bubbles, and between areas that meet.
    #[schemars(range(min = 0, max = 32))]
    pub spacing: u32,
    /// Height of the idle island and of bubbles. Windows make room for this
    /// much.
    #[schemars(range(min = 20, max = 64))]
    pub idle_height: u32,
    #[schemars(range(min = 4, max = 32))]
    pub padding: u32,
    /// Largest corner radius. Short islands are fully rounded pills.
    #[schemars(range(min = 0, max = 48))]
    pub max_radius: u32,
    /// Corners of small controls: chips, badges, icon buttons.
    #[schemars(range(min = 0, max = 24))]
    pub radius_small: u32,
    /// Corners of rows and buttons.
    #[schemars(range(min = 0, max = 32))]
    pub radius_medium: u32,
    /// Corners of cards and tiles.
    #[schemars(range(min = 0, max = 40))]
    pub radius_large: u32,
    /// The tallest the island can grow, in pixels.
    #[schemars(range(min = 200, max = 1600))]
    pub surface_height: u32,
    pub notch: Notch,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            mode: Mode::Island,
            anchor: Anchor::Top,
            island: Area::Center,
            margin: 6,
            spacing: 8,
            idle_height: 34,
            padding: 14,
            max_radius: 34,
            radius_small: 8,
            radius_medium: 14,
            radius_large: 20,
            surface_height: 640,
            notch: Notch::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Floats apart from the edge, rounded all around.
    #[default]
    Island,
    /// Attached to the edge, with concave corners flaring into it.
    Notch,
}

/// The screen edge everything sits against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    #[default]
    Top,
    Bottom,
}

/// Settings that only apply in notch mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Notch {
    /// Radius of the concave corners where the notch meets the edge.
    #[schemars(range(min = 0, max = 32))]
    pub ear_radius: u32,
}

impl Default for Notch {
    fn default() -> Self {
        Self { ear_radius: 10 }
    }
}

/// Animation constants. The spring values feed QML's `SpringAnimation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Motion {
    /// Turns animations off: views appear and change at once, and the
    /// island takes its new shape without a spring.
    pub reduced: bool,
    /// How fast every animation runs: 2 is twice as fast, 0.5 half.
    #[schemars(range(min = 0.25, max = 3.0))]
    pub speed: f64,
    #[schemars(range(min = 0.5, max = 12.0))]
    pub spring: f64,
    /// Between 0 and 1. Lower values bounce more.
    #[schemars(range(min = 0.05, max = 1.0))]
    pub damping: f64,
    #[schemars(range(min = 0, max = 1000))]
    pub fade_in_ms: u32,
    #[schemars(range(min = 0, max = 1000))]
    pub fade_out_ms: u32,
    /// How long the new view waits before fading in, so the shape moves
    /// first.
    #[schemars(range(min = 0, max = 500))]
    pub fade_delay_ms: u32,
    /// Small state changes: colors, hovers.
    #[schemars(range(min = 0, max = 600))]
    pub fast_ms: u32,
    /// Things moving or resizing inside a view, with a slight overshoot.
    #[schemars(range(min = 0, max = 1200))]
    pub move_ms: u32,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            reduced: false,
            speed: 1.0,
            spring: 4.0,
            damping: 0.32,
            fade_in_ms: 220,
            fade_out_ms: 120,
            fade_delay_ms: 90,
            fast_ms: 150,
            move_ms: 350,
        }
    }
}

/// The type scale, in pixels: four sizes and one for big numbers. An empty
/// `family` uses Inter, which Mochi's packages bring, or the system font
/// without it; an empty `display_family` keeps `family`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Text {
    #[schemars(extend("format" = "font"))]
    pub family: String,
    /// For the clocks, on the idle island and in the hub.
    #[schemars(extend("format" = "font"))]
    pub display_family: String,
    /// Labels, metadata and fine print.
    #[schemars(range(min = 8, max = 20))]
    pub caption: u32,
    /// Everything else: rows, buttons, messages.
    #[schemars(range(min = 9, max = 24))]
    pub body: u32,
    /// Card, track and section titles.
    #[schemars(range(min = 10, max = 32))]
    pub title: u32,
    /// Page titles.
    #[schemars(range(min = 12, max = 48))]
    pub headline: u32,
    /// Big numbers, like a clock.
    #[schemars(range(min = 20, max = 96))]
    pub display: u32,
    /// Before 0.0.7's scale: read as `caption`. Not sent to the UI.
    #[serde(skip_serializing)]
    #[schemars(skip)]
    pub label: Option<u32>,
    /// Before 0.0.7's scale: read as `body`. Not sent to the UI.
    #[serde(skip_serializing)]
    #[schemars(skip)]
    pub subtitle: Option<u32>,
}

impl Text {
    /// Applies the keys the scale replaced, and says which ones were used.
    pub fn migrate(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        if let Some(label) = self.label {
            self.caption = label;
            notes.push("`text.label` is now `text.caption`".to_owned());
        }
        if let Some(subtitle) = self.subtitle {
            self.body = subtitle;
            notes.push("`text.subtitle` is now `text.body`".to_owned());
        }
        notes
    }
}

impl Default for Text {
    fn default() -> Self {
        Self {
            family: String::new(),
            display_family: String::new(),
            caption: 11,
            body: 13,
            title: 15,
            headline: 20,
            display: 42,
            label: None,
            subtitle: None,
        }
    }
}

/// A color in the form QML accepts: `#rrggbb` or `#aarrggbb`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color(String);

impl Color {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// For the defaults above, which are known to be valid.
    fn fixed(value: &str) -> Self {
        Self::try_from(value.to_owned()).expect("default colors are valid")
    }
}

impl JsonSchema for Color {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Color".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "format": "color" })
    }
}

impl TryFrom<String> for Color {
    type Error = ColorError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let digits = value.strip_prefix('#').ok_or(ColorError(value.clone()))?;
        let valid = matches!(digits.len(), 6 | 8) && digits.bytes().all(|b| b.is_ascii_hexdigit());
        if !valid {
            return Err(ColorError(value));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
}

impl From<Color> for String {
    fn from(color: Color) -> Self {
        color.0
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a color, expected #rrggbb or #aarrggbb")]
pub struct ColorError(String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_accept_both_qml_forms() {
        assert!(Color::try_from("#1a2B3c".to_owned()).is_ok());
        assert!(Color::try_from("#801a2b3c".to_owned()).is_ok());
        assert_eq!(
            Color::try_from("#ABCDEF".to_owned()).unwrap().as_str(),
            "#abcdef"
        );
    }

    #[test]
    fn colors_reject_other_forms() {
        for value in ["1a2b3c", "#abc", "#1a2b3g", "red", "#1a2b3c4"] {
            assert!(Color::try_from(value.to_owned()).is_err(), "{value}");
        }
    }

    #[test]
    fn missing_sections_and_fields_keep_their_defaults() {
        let theme: Theme =
            serde_json::from_str(r##"{"colors":{"accent":"#30d158"},"motion":{"damping":0.5}}"##)
                .unwrap();
        assert_eq!(theme.colors.accent.as_str(), "#30d158");
        assert_eq!(theme.colors.background, Colors::default().background);
        assert_eq!(theme.motion.damping, 0.5);
        assert_eq!(theme.motion.spring, Motion::default().spring);
        assert_eq!(theme.layout, Layout::default());
    }

    #[test]
    fn layout_reads_modes_anchors_and_the_old_margin_name() {
        let theme: Theme = serde_json::from_str(
            r#"{"layout":{"mode":"notch","anchor":"bottom","island":"left","top_margin":4,"notch":{"ear_radius":8}}}"#,
        )
        .unwrap();
        assert_eq!(theme.layout.mode, Mode::Notch);
        assert_eq!(theme.layout.anchor, Anchor::Bottom);
        assert_eq!(theme.layout.island, Area::Left);
        assert_eq!(theme.layout.margin, 4);
        assert_eq!(theme.layout.notch.ear_radius, 8);

        let error = serde_json::from_str::<Theme>(r#"{"layout":{"anchor":"top-left"}}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("top-left"), "{error}");
    }

    #[test]
    fn unknown_tokens_are_rejected() {
        let error = serde_json::from_str::<Theme>(r##"{"colors":{"acent":"#30d158"}}"##)
            .unwrap_err()
            .to_string();
        assert!(error.contains("acent"), "{error}");
    }
}

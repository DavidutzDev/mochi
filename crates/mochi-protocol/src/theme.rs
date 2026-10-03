//! Design tokens the UI reads from its `Theme` singleton. Users override them
//! in `theme.toml`; anything they leave out keeps the default below.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub colors: Colors,
    pub layout: Layout,
    pub motion: Motion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    pub background: Color,
    pub surface: Color,
    pub foreground: Color,
    pub muted: Color,
    pub accent: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            background: Color::fixed("#e60c0c0f"),
            surface: Color::fixed("#26ffffff"),
            foreground: Color::fixed("#f5f5f7"),
            muted: Color::fixed("#98989f"),
            accent: Color::fixed("#ff9f0a"),
        }
    }
}

/// Sizes in logical pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layout {
    /// Gap between the screen edge and the island.
    pub top_margin: u32,
    /// Height of the idle island. Windows make room for this much.
    pub idle_height: u32,
    pub padding: u32,
    /// Largest corner radius. Short islands are fully rounded pills.
    pub max_radius: u32,
    /// Height of the transparent surface the island grows inside. Nothing
    /// can be taller than this.
    pub surface_height: u32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            top_margin: 6,
            idle_height: 34,
            padding: 14,
            max_radius: 26,
            surface_height: 640,
        }
    }
}

/// Animation constants. The spring values feed QML's `SpringAnimation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Motion {
    pub spring: f64,
    /// Between 0 and 1. Lower values bounce more.
    pub damping: f64,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    /// How long the new view waits before fading in, so the shape moves
    /// first.
    pub fade_delay_ms: u32,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            spring: 4.0,
            damping: 0.32,
            fade_in_ms: 220,
            fade_out_ms: 120,
            fade_delay_ms: 90,
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
    fn unknown_tokens_are_rejected() {
        let error = serde_json::from_str::<Theme>(r##"{"colors":{"acent":"#30d158"}}"##)
            .unwrap_err()
            .to_string();
        assert!(error.contains("acent"), "{error}");
    }
}

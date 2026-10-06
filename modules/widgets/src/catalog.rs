//! The widgets modules offer, read from their contributions:
//!
//! ```toml
//! [[contributions]]
//! target = "widgets"
//! kind = "widget"
//! id = "clock"
//! view = "Clock"
//! title = "Clock"
//! options = { size = [14, 8], min = [8, 5], max = [40, 20], settings = [
//!     { name = "seconds", kind = "bool", default = false, description = "Show seconds" },
//! ] }
//! ```
//!
//! Sizes are in grid cells. `frame = false` leaves the card behind the view
//! out, for a widget that floats on the wallpaper. `forget` names an action
//! the offering module runs with an instance's id when it's removed, to
//! drop what it kept for it.

use mochi_core::Contribution;
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_SIZE: (u32, u32) = (12, 8);
const SMALLEST: (u32, u32) = (2, 2);
const LARGEST: (u32, u32) = (200, 120);

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    String,
    Int,
    Float,
    Bool,
    Choice,
}

/// A setting a widget declares, which the editor shows as a field.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Setting {
    pub name: String,
    #[serde(default = "string_kind")]
    pub kind: Kind,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub default: Value,
    #[serde(default)]
    pub description: String,
}

fn string_kind() -> Kind {
    Kind::String
}

impl Setting {
    /// What `set` typed, as this setting's kind.
    pub fn parse(&self, text: &str) -> Result<toml::Value, String> {
        let wrong = |what: &str| format!("{} takes {what}, not {text:?}", self.name);
        Ok(match self.kind {
            Kind::String => toml::Value::String(text.to_owned()),
            Kind::Int => toml::Value::Integer(text.parse().map_err(|_| wrong("a whole number"))?),
            Kind::Float => toml::Value::Float(text.parse().map_err(|_| wrong("a number"))?),
            Kind::Bool => toml::Value::Boolean(match text {
                "true" | "on" | "yes" => true,
                "false" | "off" | "no" => false,
                _ => return Err(wrong("true or false")),
            }),
            Kind::Choice => {
                if !self.choices.iter().any(|choice| choice == text) {
                    return Err(wrong(&format!("one of {}", self.choices.join(", "))));
                }
                toml::Value::String(text.to_owned())
            }
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "kind": match self.kind {
                Kind::String => "string",
                Kind::Int => "int",
                Kind::Float => "float",
                Kind::Bool => "bool",
                Kind::Choice => "choice",
            },
            "choices": self.choices,
            "default": self.default,
            "description": self.description,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub module: String,
    pub widget: String,
    pub title: String,
    pub icon: Option<String>,
    pub view: String,
    pub size: (u32, u32),
    pub min: (u32, u32),
    pub max: (u32, u32),
    pub frame: bool,
    pub settings: Vec<Setting>,
    pub forget: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Options {
    size: Option<(u32, u32)>,
    min: Option<(u32, u32)>,
    max: Option<(u32, u32)>,
    frame: Option<bool>,
    settings: Vec<Setting>,
    forget: Option<String>,
}

impl Spec {
    pub fn from_offer(offer: &Contribution) -> Result<Self, String> {
        let options: Options = if offer.options.is_null() {
            Options::default()
        } else {
            serde_json::from_value(offer.options.clone()).map_err(|error| error.to_string())?
        };
        let min = clamp(options.min.unwrap_or(SMALLEST), SMALLEST, LARGEST);
        let max = clamp(options.max.unwrap_or(LARGEST), min, LARGEST);
        let size = clamp(options.size.unwrap_or(DEFAULT_SIZE), min, max);
        Ok(Self {
            module: offer.module.clone(),
            widget: offer.id.clone(),
            title: offer.title.clone(),
            icon: offer.icon.clone(),
            view: offer.view.clone(),
            size,
            min,
            max,
            frame: options.frame.unwrap_or(true),
            settings: options.settings,
            forget: options.forget,
        })
    }

    /// Fits a size between this widget's smallest and largest.
    pub fn fit(&self, size: (u32, u32)) -> (u32, u32) {
        clamp(size, self.min, self.max)
    }

    pub fn setting(&self, name: &str) -> Result<&Setting, String> {
        self.settings
            .iter()
            .find(|setting| setting.name == name)
            .ok_or_else(|| {
                let names: Vec<&str> = self
                    .settings
                    .iter()
                    .map(|setting| setting.name.as_str())
                    .collect();
                if names.is_empty() {
                    format!("{} has no settings", self.title)
                } else {
                    format!(
                        "{} has no setting {name}; it has {}",
                        self.title,
                        names.join(", ")
                    )
                }
            })
    }

    /// An instance's settings: the defaults, with what it set on top.
    pub fn settings_for(&self, set: &toml::Table) -> Value {
        let mut settings = serde_json::Map::new();
        for setting in &self.settings {
            settings.insert(setting.name.clone(), setting.default.clone());
        }
        for (key, value) in set {
            if let Ok(value) = serde_json::to_value(value) {
                settings.insert(key.clone(), value);
            }
        }
        Value::Object(settings)
    }

    pub fn to_json(&self) -> Value {
        json!({
            "module": self.module,
            "widget": self.widget,
            "title": self.title,
            "icon": self.icon,
            "view": self.view,
            "size": [self.size.0, self.size.1],
            "min": [self.min.0, self.min.1],
            "max": [self.max.0, self.max.1],
            "frame": self.frame,
            "settings": self.settings.iter().map(Setting::to_json).collect::<Vec<_>>(),
        })
    }
}

fn clamp(size: (u32, u32), min: (u32, u32), max: (u32, u32)) -> (u32, u32) {
    (
        size.0.clamp(min.0, max.0.max(min.0)),
        size.1.clamp(min.1, max.1.max(min.1)),
    )
}

/// The widgets in `offers`, without the broken ones, which are logged.
pub fn read(offers: &[Contribution]) -> Vec<Spec> {
    offers
        .iter()
        .filter(|offer| offer.kind == "widget")
        .filter_map(|offer| match Spec::from_offer(offer) {
            Ok(spec) => Some(spec),
            Err(error) => {
                tracing::warn!(module = %offer.module, widget = %offer.id, %error, "the widget's options don't read");
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(options: Value) -> Contribution {
        Contribution {
            module: "widgets".into(),
            target: "widgets".into(),
            kind: "widget".into(),
            id: "clock".into(),
            view: "Clock".into(),
            title: "Clock".into(),
            icon: None,
            order: 0,
            options,
        }
    }

    #[test]
    fn reads_sizes_and_settings_with_defaults() {
        let spec = Spec::from_offer(&offer(json!({
            "size": [14, 8],
            "min": [8, 5],
            "settings": [
                { "name": "seconds", "kind": "bool", "default": false },
                { "name": "hours", "kind": "choice", "choices": ["24", "12"], "default": "24" },
            ],
        })))
        .unwrap();
        assert_eq!((spec.size, spec.min, spec.max), ((14, 8), (8, 5), LARGEST));
        assert!(spec.frame);
        assert_eq!(spec.fit((3, 100)), (8, 100));

        let bare = Spec::from_offer(&offer(Value::Null)).unwrap();
        assert_eq!(bare.size, DEFAULT_SIZE);

        let mut set = toml::Table::new();
        set.insert("seconds".into(), toml::Value::Boolean(true));
        assert_eq!(
            spec.settings_for(&set),
            json!({ "seconds": true, "hours": "24" })
        );
    }

    #[test]
    fn parses_typed_settings() {
        let spec = Spec::from_offer(&offer(json!({ "settings": [
            { "name": "hours", "kind": "choice", "choices": ["24", "12"] },
            { "name": "seconds", "kind": "bool" },
            { "name": "count", "kind": "int" },
            { "name": "zone" },
        ] })))
        .unwrap();
        let parse =
            |name: &str, text: &str| spec.setting(name).and_then(|setting| setting.parse(text));
        assert_eq!(parse("hours", "12"), Ok(toml::Value::String("12".into())));
        assert!(parse("hours", "13").unwrap_err().contains("one of 24, 12"));
        assert_eq!(parse("seconds", "on"), Ok(toml::Value::Boolean(true)));
        assert_eq!(parse("count", "3"), Ok(toml::Value::Integer(3)));
        assert!(parse("count", "three").is_err());
        assert_eq!(
            parse("zone", "Europe/Paris"),
            Ok(toml::Value::String("Europe/Paris".into()))
        );
        assert!(
            parse("color", "red")
                .unwrap_err()
                .contains("no setting color")
        );
    }

    #[test]
    fn leaves_out_broken_and_other_offers() {
        let mut card = offer(Value::Null);
        card.kind = "card".into();
        let broken = offer(json!({ "size": "big" }));
        assert_eq!(read(&[card, broken, offer(Value::Null)]).len(), 1);
    }
}

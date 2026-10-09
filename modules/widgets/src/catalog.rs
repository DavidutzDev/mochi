//! The widgets modules offer, read from their contributions:
//!
//! ```toml
//! [[contributions]]
//! target = "widgets"
//! kind = "widget"
//! id = "clock"
//! view = "Clock"
//! title = "Clock"
//! options = { size = [14, 8], min = [8, 5], max = [40, 20], category = "Clock", settings = [
//!     { name = "seconds", kind = "bool", default = false, description = "Show seconds" },
//! ], variants = [
//!     { id = "digital", title = "Digital", description = "Time over the date" },
//!     { id = "stacked", title = "Stacked", description = "Hour above minute", size = [9, 12] },
//! ] }
//! ```
//!
//! Sizes are in grid cells. `frame = false` leaves the card behind the view
//! out, for a widget that floats on the wallpaper. `forget` names an action
//! the offering module runs with an instance's id when it's removed, to
//! drop what it kept for it. `category` groups it in the drawer, under its
//! module's name without one; `description` is one line on what it shows.
//!
//! `variants` are the widget's looks, each with its own title, one-line
//! description, sizes and `frame`, which default to the widget's. A
//! variant can name its own `view`, and list the `settings` that apply to
//! it. A placed widget records its variant, and one without gets the
//! first, so the look a widget had before it had variants goes first.

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

/// One of a widget's looks.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub id: String,
    pub title: String,
    /// One line on what it shows, like "Time over the date".
    pub description: String,
    pub view: String,
    pub size: (u32, u32),
    pub min: (u32, u32),
    pub max: (u32, u32),
    /// Whether Mochi draws the card behind it; the widget's `frame` by
    /// default.
    pub frame: bool,
    /// The widget's settings that apply to this look; all of them when
    /// `None`.
    pub settings: Option<Vec<String>>,
}

impl Variant {
    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "title": self.title,
            "description": self.description,
            "view": self.view,
            "size": [self.size.0, self.size.1],
            "min": [self.min.0, self.min.1],
            "max": [self.max.0, self.max.1],
            "frame": self.frame,
            "settings": self.settings,
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
    /// What the drawer groups it under.
    pub category: String,
    pub description: String,
    pub size: (u32, u32),
    pub min: (u32, u32),
    pub max: (u32, u32),
    pub frame: bool,
    pub settings: Vec<Setting>,
    pub forget: Option<String>,
    /// Its looks, the default first; none for a widget with one look.
    pub variants: Vec<Variant>,
}

/// How a placed widget looks: its variant, or the widget itself when it
/// has no variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Look<'a> {
    pub variant: Option<&'a Variant>,
    pub view: &'a str,
    pub size: (u32, u32),
    pub min: (u32, u32),
    pub max: (u32, u32),
    pub frame: bool,
}

impl Look<'_> {
    /// Fits a size between this look's smallest and largest.
    pub fn fit(&self, size: (u32, u32)) -> (u32, u32) {
        clamp(size, self.min, self.max)
    }
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
    category: Option<String>,
    description: Option<String>,
    variants: Vec<VariantOptions>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantOptions {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    view: Option<String>,
    size: Option<(u32, u32)>,
    min: Option<(u32, u32)>,
    max: Option<(u32, u32)>,
    frame: Option<bool>,
    settings: Option<Vec<String>>,
}

impl Spec {
    pub fn from_offer(offer: &Contribution) -> Result<Self, String> {
        let options: Options = if offer.options.is_null() {
            Options::default()
        } else {
            serde_json::from_value(offer.options.clone()).map_err(|error| error.to_string())?
        };
        let (size, min, max) = sizes(options.size, options.min, options.max);
        let frame = options.frame.unwrap_or(true);
        let mut variants: Vec<Variant> = Vec::new();
        for variant in options.variants {
            if variant.id.is_empty() || variant.id.contains([':', ' ']) {
                return Err(format!(
                    "a variant's id is one word without a colon, not {:?}",
                    variant.id
                ));
            }
            if variants.iter().any(|other| other.id == variant.id) {
                return Err(format!("two variants have the id {:?}", variant.id));
            }
            let declared = |name: &String| options.settings.iter().any(|it| &it.name == name);
            if let Some(unknown) = variant
                .settings
                .iter()
                .flatten()
                .find(|name| !declared(name))
            {
                return Err(format!(
                    "the variant {} lists the setting {unknown}, which the widget doesn't declare",
                    variant.id
                ));
            }
            let (size, min, max) = sizes(
                variant.size.or(options.size),
                variant.min.or(options.min),
                variant.max.or(options.max),
            );
            variants.push(Variant {
                id: variant.id,
                title: variant.title,
                description: variant.description,
                view: variant.view.unwrap_or_else(|| offer.view.clone()),
                size,
                min,
                max,
                frame: variant.frame.unwrap_or(frame),
                settings: variant.settings,
            });
        }
        Ok(Self {
            module: offer.module.clone(),
            widget: offer.id.clone(),
            title: offer.title.clone(),
            icon: offer.icon.clone(),
            view: offer.view.clone(),
            category: options
                .category
                .unwrap_or_else(|| mochi_core::options::title(&offer.module)),
            description: options.description.unwrap_or_default(),
            size,
            min,
            max,
            frame,
            settings: options.settings,
            forget: options.forget,
            variants,
        })
    }

    /// How a widget placed with `variant` looks: that variant, or the first
    /// when it names none or one the widget no longer has.
    pub fn look(&self, variant: Option<&str>) -> Look<'_> {
        let chosen = variant
            .and_then(|id| self.variants.iter().find(|known| known.id == id))
            .or_else(|| self.variants.first());
        match chosen {
            Some(variant) => Look {
                variant: Some(variant),
                view: &variant.view,
                size: variant.size,
                min: variant.min,
                max: variant.max,
                frame: variant.frame,
            },
            None => Look {
                variant: None,
                view: &self.view,
                size: self.size,
                min: self.min,
                max: self.max,
                frame: self.frame,
            },
        }
    }

    /// The variant called `id`, or why there's none.
    pub fn variant(&self, id: &str) -> Result<&Variant, String> {
        self.variants
            .iter()
            .find(|variant| variant.id == id)
            .ok_or_else(|| {
                if self.variants.is_empty() {
                    return format!("{} has one look, without variants", self.title);
                }
                let ids: Vec<&str> = self
                    .variants
                    .iter()
                    .map(|variant| variant.id.as_str())
                    .collect();
                format!(
                    "{} has no variant {id}; it has {}",
                    self.title,
                    ids.join(", ")
                )
            })
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
            "category": self.category,
            "description": self.description,
            "size": [self.size.0, self.size.1],
            "min": [self.min.0, self.min.1],
            "max": [self.max.0, self.max.1],
            "frame": self.frame,
            "settings": self.settings.iter().map(Setting::to_json).collect::<Vec<_>>(),
            "variants": self.variants.iter().map(Variant::to_json).collect::<Vec<_>>(),
        })
    }
}

/// The size it's added at, and its smallest and largest, from what was
/// declared, each within the others.
fn sizes(
    size: Option<(u32, u32)>,
    min: Option<(u32, u32)>,
    max: Option<(u32, u32)>,
) -> ((u32, u32), (u32, u32), (u32, u32)) {
    let min = clamp(min.unwrap_or(SMALLEST), SMALLEST, LARGEST);
    let max = clamp(max.unwrap_or(LARGEST), min, LARGEST);
    (clamp(size.unwrap_or(DEFAULT_SIZE), min, max), min, max)
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
        assert_eq!(spec.look(None).fit((3, 100)), (8, 100));
        // Without a category, its module's name.
        assert_eq!(spec.category, "Widgets");

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
    fn reads_variants() {
        let spec = Spec::from_offer(&offer(json!({
            "size": [14, 8],
            "min": [8, 5],
            "category": "Clock",
            "settings": [{ "name": "seconds", "kind": "bool" }, { "name": "zones" }],
            "variants": [
                { "id": "digital", "title": "Digital", "description": "Time over the date" },
                { "id": "stacked", "title": "Stacked", "size": [9, 12], "min": [6, 6], "settings": ["seconds"] },
                { "id": "analog", "title": "Analog", "view": "Analog", "size": [10, 10] },
                { "id": "minimal", "title": "Minimal", "frame": false },
            ],
        })))
        .unwrap();
        assert_eq!(spec.category, "Clock");
        assert_eq!(spec.variants.len(), 4);
        // Sizes default to the widget's, and views to its own.
        let digital = spec.look(Some("digital"));
        assert_eq!(
            (digital.size, digital.min, digital.view),
            ((14, 8), (8, 5), "Clock")
        );
        let stacked = spec.look(Some("stacked"));
        assert_eq!((stacked.size, stacked.min), ((9, 12), (6, 6)));
        assert_eq!(stacked.fit((2, 2)), (6, 6));
        assert_eq!(spec.look(Some("analog")).view, "Analog");
        // A look can go without the card; the others have the widget's.
        assert!(!spec.look(Some("minimal")).frame);
        assert!(spec.look(Some("analog")).frame);
        // None, or one that's gone, is the first.
        assert_eq!(spec.look(None).variant.unwrap().id, "digital");
        assert_eq!(spec.look(Some("gone")).variant.unwrap().id, "digital");
        assert!(
            spec.variant("gone")
                .unwrap_err()
                .contains("digital, stacked, analog, minimal")
        );

        let json = spec.to_json();
        assert_eq!(json["variants"][1]["size"], json!([9, 12]));
        assert_eq!(json["variants"][1]["settings"], json!(["seconds"]));
        assert_eq!(json["variants"][0]["settings"], Value::Null);

        // Without variants: the widget's own look.
        let bare = Spec::from_offer(&offer(json!({ "size": [14, 8] }))).unwrap();
        let look = bare.look(Some("stacked"));
        assert_eq!(
            (look.variant, look.size, look.view),
            (None, (14, 8), "Clock")
        );
        assert!(bare.variant("stacked").unwrap_err().contains("one look"));
    }

    #[test]
    fn refuses_broken_variants() {
        let twice = offer(json!({ "variants": [
            { "id": "a", "title": "A" }, { "id": "a", "title": "A again" },
        ] }));
        assert!(
            Spec::from_offer(&twice)
                .unwrap_err()
                .contains("two variants")
        );
        let colon = offer(json!({ "variants": [{ "id": "a:b", "title": "A" }] }));
        assert!(Spec::from_offer(&colon).is_err());
        let unknown = offer(json!({ "variants": [
            { "id": "a", "title": "A", "settings": ["nope"] },
        ] }));
        assert!(Spec::from_offer(&unknown).unwrap_err().contains("nope"));
        let typo = offer(json!({ "variants": [{ "id": "a", "title": "A", "sise": [2, 2] }] }));
        assert!(Spec::from_offer(&typo).is_err());
    }

    #[test]
    fn leaves_out_broken_and_other_offers() {
        let mut card = offer(Value::Null);
        card.kind = "card".into();
        let broken = offer(json!({ "size": "big" }));
        assert_eq!(read(&[card, broken, offer(Value::Null)]).len(), 1);
    }
}

//! Every option the settings panel shows: a [`Section`] per page, and a
//! [`Field`] per option in it.
//!
//! A field comes from three places. The settings type's JSON schema
//! ([`schema_of`]) gives its kind, its choices and its range. The example
//! file gives its description, from the comment above its `# key = value`
//! line, and the order fields show in. The defaults table gives its
//! default.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value as Json};

use crate::examples::is_default;

/// A settings type's JSON schema, subschemas inlined.
pub fn schema_of<T: JsonSchema>() -> Json {
    let generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator();
    generator.into_root_schema_for::<T>().to_value()
}

/// Where a section shows in the panel's sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    Appearance,
    Shell,
    Modules,
    Plugins,
}

/// One page of the panel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    /// What `mochi ipc settings open` takes: `colors`, `island`, `osd`.
    pub id: String,
    /// Where its fields live, from the file: `theme.colors`,
    /// `config.module.osd`.
    pub path: String,
    pub title: String,
    pub description: String,
    pub group: Group,
    pub icon: String,
    /// The module it sets, for module and plugin pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A heading over the fields of a table, like `cpu` over `cpu.notice`.
    Group,
    Bool,
    Int,
    Float,
    Text,
    /// `#rrggbb` or `#aarrggbb`.
    Color,
    /// A font family installed on the system; empty for the default.
    Font,
    /// One of `choices`.
    Choice,
    /// A list of `items`.
    List,
    /// Anything else, edited as TOML.
    Table,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    pub value: String,
    /// What the menu shows; the value, in words, without one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Swatches to show beside it, like a theme preset's colors.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<String>,
}

/// One option.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    /// From the file: `config.module.osd.timeout_ms`.
    pub path: String,
    pub title: String,
    pub description: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// What a list holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Kind>,
    /// Where its values come from, for a text or a list the panel offers
    /// as a menu: `x-source` in the schema. `command` is a module's action
    /// and its arguments; `app`, `audio-device`, `tray-app`, `player` and
    /// `hub-card` come from what runs now. Typing anything else still works.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Ready-made values for a command, like `["hyprlock"]`: `x-suggest` in
    /// the schema. The daemon keeps the ones whose program is installed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<Json>,
    /// It may be left unset, which lets the module decide.
    pub optional: bool,
    /// `null` when unset by default.
    pub default: Json,
}

/// The prose of an example file: the comment above each `# key = value`
/// line and above each `[table]`, by dotted path from the file's root, and
/// the order the keys come in.
#[derive(Debug, Clone, Default)]
pub struct Comments {
    notes: HashMap<String, String>,
    order: Vec<String>,
}

impl Comments {
    pub fn parse(example: &str) -> Self {
        let mut comments = Self::default();
        let mut table = String::new();
        let mut prose = String::new();
        for line in example.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                prose.clear();
            } else if let Some(name) = trimmed
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                table = name.trim().to_owned();
                comments.note(&table, &mut prose);
            } else if let Some(rest) = trimmed.strip_prefix('#') {
                let rest = rest.strip_prefix(' ').unwrap_or(rest);
                if is_default(rest) {
                    let key = rest.split_once(" = ").expect("a default").0;
                    let path = if table.is_empty() {
                        key.to_owned()
                    } else {
                        format!("{table}.{key}")
                    };
                    comments.order.push(path.clone());
                    comments.note(&path, &mut prose);
                } else if rest.is_empty() || rest.starts_with(' ') {
                    // A paragraph break, or an indented example: neither
                    // fits in a row's description.
                } else {
                    if !prose.is_empty() {
                        prose.push(' ');
                    }
                    prose.push_str(rest);
                }
            }
        }
        comments
    }

    fn note(&mut self, path: &str, prose: &mut String) {
        if !prose.is_empty() {
            self.notes.insert(path.to_owned(), std::mem::take(prose));
        }
    }

    /// The comment above `path`.
    pub fn get(&self, path: &str) -> Option<&str> {
        self.notes.get(path).map(String::as_str)
    }

    /// Where `path` comes in the example, for sorting.
    fn position(&self, path: &str) -> usize {
        self.order
            .iter()
            .position(|known| known == path || known.starts_with(&format!("{path}.")))
            .unwrap_or(usize::MAX)
    }

    /// The section's title and description from the comment above its
    /// table, like `OSD: shows volume, mute, ...`.
    pub fn heading(&self, table: &str) -> Option<(String, String)> {
        let note = self.get(table)?;
        match note.split_once(": ") {
            Some((title, rest)) if title.len() <= 24 && !title.contains('.') => {
                Some((title.to_owned(), capitalize(rest)))
            }
            _ => Some((String::new(), note.to_owned())),
        }
    }
}

/// The fields of a table: `schema` is its JSON schema, `file` and `table`
/// say where it is (`config`, `module.osd`), `comments` come from its
/// example and `defaults` is the whole file at its defaults.
pub fn fields(
    schema: &Json,
    file: &str,
    table: &str,
    comments: &Comments,
    defaults: &toml::Table,
) -> Vec<Field> {
    let mut out = Vec::new();
    collect(schema, file, table, comments, defaults, &mut out);
    out
}

fn collect(
    schema: &Json,
    file: &str,
    table: &str,
    comments: &Comments,
    defaults: &toml::Table,
    out: &mut Vec<Field>,
) {
    let Some(properties) = schema.get("properties").and_then(Json::as_object) else {
        return;
    };
    let mut keys: Vec<&String> = properties.keys().collect();
    let path_of = |key: &str| {
        if table.is_empty() {
            key.to_owned()
        } else {
            format!("{table}.{key}")
        }
    };
    keys.sort_by_key(|key| comments.position(&path_of(key)));
    for key in keys {
        let property = &properties[key];
        let path = path_of(key);
        let shape = Shape::of(property);
        // A table's doc comment describes its type, not this option.
        let description = comments
            .get(&path)
            .map(str::to_owned)
            .or_else(|| {
                shape
                    .description
                    .clone()
                    .filter(|_| shape.kind != Kind::Group)
            })
            .unwrap_or_default();
        let parts: Vec<&str> = path.split('.').collect();
        let default = crate::changes::get(defaults, &parts)
            .map(to_json)
            .or_else(|| property.get("default").cloned())
            .unwrap_or(Json::Null);
        let field = Field {
            path: format!("{file}.{path}"),
            title: title(key),
            description,
            kind: shape.kind,
            choices: shape.choices,
            min: shape.min,
            max: shape.max,
            items: shape.items,
            source: shape.source,
            suggestions: shape.suggestions,
            optional: shape.optional,
            default,
        };
        out.push(field);
        if shape.kind == Kind::Group {
            collect(shape.schema, file, &path, comments, defaults, out);
        }
    }
}

/// What a property's schema says about it.
struct Shape<'a> {
    kind: Kind,
    optional: bool,
    choices: Vec<Choice>,
    min: Option<f64>,
    max: Option<f64>,
    items: Option<Kind>,
    source: Option<String>,
    suggestions: Vec<Json>,
    description: Option<String>,
    /// The schema without its `null`, for a group's own fields.
    schema: &'a Json,
}

impl<'a> Shape<'a> {
    fn of(schema: &'a Json) -> Self {
        let description = schema
            .get("description")
            .and_then(Json::as_str)
            .map(str::to_owned);
        let (schema, optional) = without_null(schema);
        let mut shape = Self {
            kind: Kind::Table,
            optional,
            choices: Vec::new(),
            min: None,
            max: None,
            items: None,
            source: schema
                .get("x-source")
                .or_else(|| schema.get("items").and_then(|items| items.get("x-source")))
                .and_then(Json::as_str)
                .map(str::to_owned),
            suggestions: schema
                .get("x-suggest")
                .and_then(Json::as_array)
                .cloned()
                .unwrap_or_default(),
            description,
            schema,
        };
        if let Some(choices) = choices(schema) {
            shape.kind = Kind::Choice;
            shape.choices = choices;
            return shape;
        }
        let types = types(schema);
        let format = schema.get("format").and_then(Json::as_str).unwrap_or("");
        match types.iter().find(|kind| **kind != "null").copied() {
            Some("boolean") => shape.kind = Kind::Bool,
            Some("integer") => {
                shape.kind = Kind::Int;
                (shape.min, shape.max) = range(schema, format);
            }
            Some("number") => {
                shape.kind = Kind::Float;
                (shape.min, shape.max) = range(schema, format);
            }
            Some("string") if format == "color" => shape.kind = Kind::Color,
            Some("string") if format == "font" => shape.kind = Kind::Font,
            Some("string") => shape.kind = Kind::Text,
            Some("array") => {
                let items = schema.get("items").map(Shape::of);
                match items.as_ref().map(|items| items.kind) {
                    Some(kind @ (Kind::Text | Kind::Int | Kind::Float | Kind::Choice)) => {
                        shape.kind = Kind::List;
                        shape.items = Some(kind);
                        // A list of fixed values picks several of them.
                        if kind == Kind::Choice {
                            shape.choices = items.map(|items| items.choices).unwrap_or_default();
                        }
                    }
                    _ => shape.kind = Kind::Table,
                }
            }
            Some("object") => {
                let open = schema
                    .get("additionalProperties")
                    .is_some_and(|extra| extra != &Json::Bool(false));
                if schema.get("properties").is_some() && !open {
                    shape.kind = Kind::Group;
                }
            }
            _ => {}
        }
        shape.optional |= types.contains(&"null");
        shape
    }
}

/// The schema of an `Option<T>` as `T`'s, and whether it was one.
fn without_null(schema: &Json) -> (&Json, bool) {
    for combinator in ["anyOf", "oneOf"] {
        if let Some(branches) = schema.get(combinator).and_then(Json::as_array) {
            let rest: Vec<&Json> = branches
                .iter()
                .filter(|branch| branch.get("type").and_then(Json::as_str) != Some("null"))
                .collect();
            if rest.len() < branches.len() && rest.len() == 1 {
                return (rest[0], true);
            }
        }
    }
    (schema, false)
}

fn types(schema: &Json) -> Vec<&str> {
    match schema.get("type") {
        Some(Json::String(kind)) => vec![kind.as_str()],
        Some(Json::Array(kinds)) => kinds.iter().filter_map(Json::as_str).collect(),
        _ => Vec::new(),
    }
}

/// An enum's values, with the description of each where it has one.
fn choices(schema: &Json) -> Option<Vec<Choice>> {
    let plain = |values: &Json, description: &str| -> Option<Vec<Choice>> {
        values
            .as_array()?
            .iter()
            .map(|value| {
                Some(Choice {
                    value: value.as_str()?.to_owned(),
                    label: String::new(),
                    description: description.to_owned(),
                    colors: Vec::new(),
                })
            })
            .collect()
    };
    if let Some(values) = schema.get("enum") {
        return plain(values, "");
    }
    let branches = schema.get("oneOf").or_else(|| schema.get("anyOf"))?;
    let mut out = Vec::new();
    for branch in branches.as_array()? {
        let description = branch
            .get("description")
            .and_then(Json::as_str)
            .unwrap_or("");
        match (branch.get("const"), branch.get("enum")) {
            (Some(Json::String(value)), _) => out.push(Choice {
                value: value.clone(),
                label: String::new(),
                description: description.to_owned(),
                colors: Vec::new(),
            }),
            (_, Some(values)) => out.extend(plain(values, description)?),
            _ => return None,
        }
    }
    Some(out)
}

/// The range of a number, without the bounds that only come from its Rust
/// type, like 255 for a `u8`: those say nothing about the option.
fn range(schema: &Json, format: &str) -> (Option<f64>, Option<f64>) {
    let bound = |name: &str| schema.get(name).and_then(Json::as_f64);
    let natural = |value: f64| match format {
        "uint8" => value == f64::from(u8::MAX),
        "uint16" => value == f64::from(u16::MAX),
        "uint32" => value == f64::from(u32::MAX),
        "int8" => value == f64::from(i8::MIN) || value == f64::from(i8::MAX),
        "int16" => value == f64::from(i16::MIN) || value == f64::from(i16::MAX),
        "int32" => value == f64::from(i32::MIN) || value == f64::from(i32::MAX),
        _ => false,
    };
    let min = bound("minimum")
        .or_else(|| bound("exclusiveMinimum"))
        .filter(|value| !natural(*value));
    let max = bound("maximum")
        .or_else(|| bound("exclusiveMaximum"))
        .filter(|value| !natural(*value));
    (min, max)
}

/// A key as a row title: `timeout_ms` reads "Timeout (ms)".
pub fn title(key: &str) -> String {
    const UNITS: [(&str, &str); 6] = [
        ("_ms", "ms"),
        ("_seconds", "s"),
        ("_minutes", "min"),
        ("_hours", "h"),
        ("_mb", "MB"),
        ("_percent", "%"),
    ];
    const ACRONYMS: [&str; 10] = [
        "cpu", "gpu", "osd", "url", "ui", "id", "dnd", "fps", "ip", "ms",
    ];
    let (stem, unit) = UNITS
        .iter()
        .find_map(|(suffix, unit)| key.strip_suffix(suffix).map(|stem| (stem, Some(*unit))))
        .unwrap_or((key, None));
    let words: Vec<String> = stem
        .split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            if ACRONYMS.contains(&word) {
                word.to_uppercase()
            } else {
                word.to_owned()
            }
        })
        .collect();
    let mut text = capitalize(&words.join(" "));
    if let Some(unit) = unit {
        text.push_str(&format!(" ({unit})"));
    }
    text
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn to_json(value: &toml::Value) -> Json {
    match value {
        toml::Value::String(text) => Json::String(text.clone()),
        toml::Value::Integer(number) => Json::from(*number),
        toml::Value::Float(number) => Json::from(*number),
        toml::Value::Boolean(flag) => Json::Bool(*flag),
        toml::Value::Datetime(time) => Json::String(time.to_string()),
        toml::Value::Array(items) => Json::Array(items.iter().map(to_json).collect()),
        toml::Value::Table(table) => Json::Object(
            table
                .iter()
                .map(|(key, value)| (key.clone(), to_json(value)))
                .collect::<Map<_, _>>(),
        ),
    }
}

/// A value the panel sent, as the TOML a field of `kind` takes. `None` for
/// `null`, which unsets it.
pub fn from_json(
    value: &Json,
    kind: Kind,
    items: Option<Kind>,
) -> Result<Option<toml::Value>, String> {
    let wrong = || format!("{value} is not a valid {}", kind_name(kind));
    Ok(Some(match (kind, value) {
        (_, Json::Null) => return Ok(None),
        (Kind::Bool, Json::Bool(flag)) => toml::Value::Boolean(*flag),
        (Kind::Int, Json::Number(number)) => {
            let integer = number.as_i64().or_else(|| {
                number
                    .as_f64()
                    .filter(|float| float.fract() == 0.0 && float.abs() < 9.0e15)
                    .map(|float| float as i64)
            });
            toml::Value::Integer(integer.ok_or_else(wrong)?)
        }
        (Kind::Float, Json::Number(number)) => {
            toml::Value::Float(number.as_f64().ok_or_else(wrong)?)
        }
        (Kind::Text | Kind::Color | Kind::Font | Kind::Choice, Json::String(text)) => {
            toml::Value::String(text.clone())
        }
        (Kind::List, Json::Array(values)) => {
            let item = items.unwrap_or(Kind::Text);
            let mut out = Vec::new();
            for value in values {
                out.extend(from_json(value, item, None)?);
            }
            toml::Value::Array(out)
        }
        (Kind::Table | Kind::Group, _) => {
            toml::Value::try_from(value).map_err(|error| error.to_string())?
        }
        _ => return Err(wrong()),
    }))
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Group | Kind::Table => "table",
        Kind::Bool => "true or false",
        Kind::Int => "whole number",
        Kind::Float => "number",
        Kind::Text => "text",
        Kind::Color => "color",
        Kind::Font => "font family",
        Kind::Choice => "choice",
        Kind::List => "list",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[derive(Debug, Default, Deserialize, JsonSchema)]
    #[serde(default, deny_unknown_fields)]
    #[allow(dead_code)]
    struct Limits {
        notice: u8,
        critical: u8,
    }

    #[derive(Debug, Default, Deserialize, JsonSchema)]
    #[serde(rename_all = "lowercase")]
    #[allow(dead_code)]
    enum Where {
        /// The focused monitor.
        #[default]
        Focus,
        Pointer,
        All,
    }

    #[derive(Debug, Default, Deserialize, JsonSchema)]
    #[serde(default, deny_unknown_fields)]
    #[allow(dead_code)]
    struct Settings {
        /// From the doc comment.
        timeout_ms: u64,
        #[schemars(range(min = 480, max = 2400))]
        width: u32,
        damping: f64,
        volume: bool,
        terminal: Vec<String>,
        area: Option<Where>,
        cpu: Limits,
        panels: Where,
        providers: BTreeMap<String, Limits>,
    }

    const EXAMPLE: &str = "\
# Clock: shows the time.
[module.clock]
# How wide it is.
# width = 860
# How long a notice stays,
# in milliseconds.
# timeout_ms = 1500
#
#   an indented example
# volume = true
# cpu = { notice = 90, critical = 98 }
";

    fn built() -> Vec<Field> {
        let comments = Comments::parse(EXAMPLE);
        let defaults: toml::Table = toml::from_str(&crate::examples::uncommented(EXAMPLE)).unwrap();
        fields(
            &schema_of::<Settings>(),
            "config",
            "module.clock",
            &comments,
            &defaults,
        )
    }

    fn field<'a>(fields: &'a [Field], path: &str) -> &'a Field {
        fields
            .iter()
            .find(|field| field.path == path)
            .unwrap_or_else(|| panic!("no {path}"))
    }

    #[test]
    fn fields_follow_the_example() {
        let fields = built();
        let paths: Vec<&str> = fields.iter().map(|field| field.path.as_str()).collect();
        assert_eq!(
            &paths[..6],
            [
                "config.module.clock.width",
                "config.module.clock.timeout_ms",
                "config.module.clock.volume",
                "config.module.clock.cpu",
                "config.module.clock.cpu.critical",
                "config.module.clock.cpu.notice",
            ]
        );

        let timeout = field(&fields, "config.module.clock.timeout_ms");
        assert_eq!(timeout.title, "Timeout (ms)");
        assert_eq!(
            timeout.description,
            "How long a notice stays, in milliseconds."
        );
        assert_eq!(timeout.kind, Kind::Int);
        assert_eq!(timeout.default, json!(1500));
        assert_eq!((timeout.min, timeout.max), (Some(0.0), None));

        let width = field(&fields, "config.module.clock.width");
        assert_eq!((width.min, width.max), (Some(480.0), Some(2400.0)));

        let notice = field(&fields, "config.module.clock.cpu.notice");
        assert_eq!(notice.default, json!(90));
        assert_eq!((notice.min, notice.max), (Some(0.0), None));
        assert_eq!(field(&fields, "config.module.clock.cpu").kind, Kind::Group);
    }

    #[test]
    fn kinds_come_from_the_schema() {
        let fields = built();
        assert_eq!(
            field(&fields, "config.module.clock.damping").kind,
            Kind::Float
        );
        assert_eq!(
            field(&fields, "config.module.clock.volume").kind,
            Kind::Bool
        );
        let terminal = field(&fields, "config.module.clock.terminal");
        assert_eq!(
            (terminal.kind, terminal.items),
            (Kind::List, Some(Kind::Text))
        );
        assert_eq!(
            field(&fields, "config.module.clock.providers").kind,
            Kind::Table
        );

        let panels = field(&fields, "config.module.clock.panels");
        assert_eq!(panels.kind, Kind::Choice);
        let mut values: Vec<&str> = panels
            .choices
            .iter()
            .map(|choice| choice.value.as_str())
            .collect();
        values.sort_unstable();
        assert_eq!(values, ["all", "focus", "pointer"]);
        assert!(
            panels
                .choices
                .iter()
                .any(|choice| choice.description == "The focused monitor.")
        );

        let area = field(&fields, "config.module.clock.area");
        assert_eq!((area.kind, area.optional), (Kind::Choice, true));
        assert_eq!(area.default, Json::Null);

        let timeout = field(&fields, "config.module.clock.timeout_ms");
        assert!(!timeout.optional);
    }

    #[test]
    fn the_heading_comes_from_above_the_table() {
        let comments = Comments::parse(EXAMPLE);
        assert_eq!(
            comments.heading("module.clock"),
            Some(("Clock".to_owned(), "Shows the time.".to_owned()))
        );
        assert_eq!(comments.get("module.clock.volume"), None);
    }

    #[test]
    fn titles_read_like_words() {
        assert_eq!(title("max_volume"), "Max volume");
        assert_eq!(title("sustain_seconds"), "Sustain (s)");
        assert_eq!(title("gpu"), "GPU");
        assert_eq!(title("click_outside"), "Click outside");
        assert_eq!(title("max_item_mb"), "Max item (MB)");
    }

    #[test]
    fn values_from_the_panel_take_the_fields_kind() {
        assert_eq!(
            from_json(&json!(4.0), Kind::Int, None),
            Ok(Some(toml::Value::Integer(4)))
        );
        assert_eq!(
            from_json(&json!(4), Kind::Float, None),
            Ok(Some(toml::Value::Float(4.0)))
        );
        assert!(from_json(&json!(4.5), Kind::Int, None).is_err());
        assert!(from_json(&json!("yes"), Kind::Bool, None).is_err());
        assert_eq!(from_json(&Json::Null, Kind::Int, None), Ok(None));
        assert_eq!(
            from_json(&json!([80, 50]), Kind::List, Some(Kind::Int)),
            Ok(Some(toml::Value::Array(vec![
                toml::Value::Integer(80),
                toml::Value::Integer(50)
            ])))
        );
    }
}

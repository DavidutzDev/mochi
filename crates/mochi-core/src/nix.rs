//! TOML values written as Nix, for the "Copy as Nix" buttons: what a user
//! pastes into `programs.mochi` in home-manager.

use std::fmt::Write as _;

/// A Nix string literal.
pub fn string(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace("${", "\\${")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

/// An attribute name: bare when Nix allows it, quoted otherwise.
pub fn key(key: &str) -> String {
    let bare = key
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '\'');
    if bare { key.to_owned() } else { string(key) }
}

/// A value on one line.
pub fn value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => string(text),
        toml::Value::Integer(number) => number.to_string(),
        toml::Value::Float(number) => {
            let text = number.to_string();
            if text.contains('.') || text.contains('e') || !number.is_finite() {
                text
            } else {
                format!("{text}.0")
            }
        }
        toml::Value::Boolean(flag) => flag.to_string(),
        toml::Value::Datetime(time) => string(&time.to_string()),
        toml::Value::Array(items) => {
            let items: Vec<String> = items.iter().map(self::value).collect();
            if items.is_empty() {
                "[ ]".to_owned()
            } else {
                format!("[ {} ]", items.join(" "))
            }
        }
        toml::Value::Table(table) => {
            let fields: Vec<String> = table
                .iter()
                .map(|(name, item)| format!("{} = {};", key(name), self::value(item)))
                .collect();
            format!("{{ {} }}", fields.join(" "))
        }
    }
}

/// `name = { ... };` over several lines, indented by `indent` spaces. A
/// table with a single entry folds into a dotted name, like
/// `island.notices = "focus";`, and long lists take a line per item.
pub fn binding(name: &str, value: &toml::Value, indent: usize) -> String {
    let mut out = String::new();
    write_binding(&mut out, &key(name), value, indent);
    out
}

fn write_binding(out: &mut String, name: &str, value: &toml::Value, indent: usize) {
    let pad = " ".repeat(indent);
    match value {
        toml::Value::Table(table) if table.len() == 1 => {
            let (inner, item) = table.iter().next().expect("one entry");
            write_binding(out, &format!("{name}.{}", key(inner)), item, indent);
        }
        toml::Value::Table(table) if !table.is_empty() => {
            let _ = writeln!(out, "{pad}{name} = {{");
            for (inner, item) in table {
                write_binding(out, &key(inner), item, indent + 2);
            }
            let _ = writeln!(out, "{pad}}};");
        }
        toml::Value::Array(items) if self::value(value).len() + name.len() + indent > 76 => {
            let _ = writeln!(out, "{pad}{name} = [");
            for item in items {
                let _ = writeln!(out, "{pad}  {}", self::value(item));
            }
            let _ = writeln!(out, "{pad}];");
        }
        _ => {
            let _ = writeln!(out, "{pad}{name} = {};", self::value(value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> toml::Value {
        toml::Value::Table(toml::from_str(text).unwrap())
    }

    #[test]
    fn strings_escape_what_nix_reads() {
        assert_eq!(string(r#"say "${hi}"\"#), r#""say \"\${hi}\"\\""#);
        assert_eq!(key("weird key"), "\"weird key\"");
        assert_eq!(key("max_volume"), "max_volume");
    }

    #[test]
    fn floats_stay_floats() {
        assert_eq!(value(&toml::Value::Float(2.0)), "2.0");
        assert_eq!(value(&toml::Value::Float(0.32)), "0.32");
    }

    #[test]
    fn single_entries_fold_into_dotted_names() {
        let settings = table(
            r#"
            modules = ["idle", "osd"]
            [island]
            notices = "focus"
            [module.audio]
            max_volume = 200
            [module.performance]
            sustain_seconds = 10
            cpu = { notice = 85, critical = 95 }
            "#,
        );
        assert_eq!(
            binding("settings", &settings, 0),
            "settings = {\n\
             \x20 island.notices = \"focus\";\n\
             \x20 module = {\n\
             \x20   audio.max_volume = 200;\n\
             \x20   performance = {\n\
             \x20     cpu = {\n\
             \x20       critical = 95;\n\
             \x20       notice = 85;\n\
             \x20     };\n\
             \x20     sustain_seconds = 10;\n\
             \x20   };\n\
             \x20 };\n\
             \x20 modules = [ \"idle\" \"osd\" ];\n\
             };\n"
        );
    }

    #[test]
    fn long_lists_take_a_line_per_item() {
        let modules: Vec<toml::Value> = (0..12)
            .map(|index| toml::Value::String(format!("module{index}")))
            .collect();
        let text = binding("modules", &toml::Value::Array(modules), 2);
        assert!(
            text.starts_with("  modules = [\n    \"module0\"\n"),
            "{text}"
        );
        assert!(text.ends_with("  ];\n"), "{text}");
    }
}

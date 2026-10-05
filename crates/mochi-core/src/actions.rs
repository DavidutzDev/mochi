//! Checks command arguments against a module's [`ActionSpec`] before the
//! module sees them, so modules only ever receive well-typed [`Args`].

use mochi_protocol::{ActionSpec, ArgKind, ArgSpec};

pub use mochi_protocol::spec::{ArgValue, Args};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArgError {
    #[error("missing <{name}>\nusage: {usage}")]
    Missing { name: String, usage: String },
    #[error("too many arguments\nusage: {usage}")]
    TooMany { usage: String },
    #[error("<{name}> must be {expected}, got {value:?}\nusage: {usage}")]
    Invalid {
        name: String,
        expected: String,
        value: String,
        usage: String,
    },
}

/// Parses words from the command line against `spec`.
pub fn parse(spec: &ActionSpec, words: &[String]) -> Result<Args, ArgError> {
    let usage = spec.usage();
    let mut args = Vec::new();
    let mut words = words.iter();

    for arg in &spec.args {
        let value = if arg.rest {
            let rest: Vec<&str> = words.by_ref().map(String::as_str).collect();
            if rest.is_empty() {
                None
            } else {
                Some(rest.join(" "))
            }
        } else {
            words.next().cloned()
        };

        match value {
            Some(word) => args.push((arg.name.clone(), convert(arg, word, &usage)?)),
            None if arg.optional => {}
            None => {
                return Err(ArgError::Missing {
                    name: arg.name.clone(),
                    usage,
                });
            }
        }
    }

    if words.next().is_some() {
        return Err(ArgError::TooMany { usage });
    }
    Ok(args.into_iter().collect())
}

/// Checks the rules `parse` relies on: only trailing arguments are optional,
/// only the last one takes the rest, and names are unique.
pub fn validate(spec: &ActionSpec) -> Result<(), String> {
    let mut optional_seen = false;
    for (index, arg) in spec.args.iter().enumerate() {
        if arg.rest && index + 1 != spec.args.len() {
            return Err(format!(
                "{}: only the last argument can take the rest",
                spec.name
            ));
        }
        if optional_seen && !arg.optional {
            return Err(format!(
                "{}: <{}> is required but follows an optional argument",
                spec.name, arg.name
            ));
        }
        optional_seen |= arg.optional;
        if spec.args[..index]
            .iter()
            .any(|other| other.name == arg.name)
        {
            return Err(format!("{}: <{}> is declared twice", spec.name, arg.name));
        }
    }
    Ok(())
}

fn convert(arg: &ArgSpec, word: String, usage: &str) -> Result<ArgValue, ArgError> {
    let invalid = |expected: &str| ArgError::Invalid {
        name: arg.name.clone(),
        expected: expected.to_owned(),
        value: word.clone(),
        usage: usage.to_owned(),
    };

    match &arg.kind {
        ArgKind::String => Ok(ArgValue::String(word)),
        ArgKind::Int => word
            .strip_prefix('+')
            .unwrap_or(&word)
            .parse()
            .map(ArgValue::Int)
            .map_err(|_| invalid("an integer")),
        ArgKind::Float => word
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(ArgValue::Float)
            .ok_or_else(|| invalid("a number")),
        ArgKind::Bool => match word.to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" => Ok(ArgValue::Bool(true)),
            "false" | "off" | "no" => Ok(ArgValue::Bool(false)),
            _ => Err(invalid("on or off")),
        },
        ArgKind::Choice { values } => {
            if values.contains(&word) {
                Ok(ArgValue::String(word))
            } else {
                Err(invalid(&format!("one of {}", values.join(", "))))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    fn volume() -> ActionSpec {
        ActionSpec::new("volume", "Change the volume")
            .arg(ArgSpec::int("delta", "Percent to add"))
            .arg(ArgSpec::bool("silent", "Skip the OSD").optional())
    }

    #[test]
    fn parses_typed_arguments() {
        let args = parse(&volume(), &words("+5 off")).unwrap();
        assert_eq!(args.int("delta"), Some(5));
        assert_eq!(args.bool("silent"), Some(false));

        let args = parse(&volume(), &words("-10")).unwrap();
        assert_eq!(args.int("delta"), Some(-10));
        assert_eq!(args.get("silent"), None);
    }

    #[test]
    fn rest_takes_every_remaining_word() {
        let spec = ActionSpec::new("show", "Show a view")
            .arg(ArgSpec::choice("view", "Which view", ["Small", "Card"]))
            .arg(ArgSpec::string("text", "Body").optional().rest());

        let args = parse(&spec, &words("Card hello there world")).unwrap();
        assert_eq!(args.str("view"), Some("Card"));
        assert_eq!(args.str("text"), Some("hello there world"));

        let args = parse(&spec, &words("Small")).unwrap();
        assert_eq!(args.str("text"), None);
    }

    #[test]
    fn errors_name_the_argument_and_show_usage() {
        let error = parse(&volume(), &[]).unwrap_err().to_string();
        assert_eq!(error, "missing <delta>\nusage: volume <delta> [silent]");

        let error = parse(&volume(), &words("loud")).unwrap_err().to_string();
        assert!(
            error.starts_with("<delta> must be an integer, got \"loud\""),
            "{error}"
        );

        let error = parse(&volume(), &words("5 on extra")).unwrap_err();
        assert!(matches!(error, ArgError::TooMany { .. }));
    }

    #[test]
    fn choices_and_floats_are_checked() {
        let spec = ActionSpec::new("set", "Set")
            .arg(ArgSpec::choice("mode", "Mode", ["on", "off"]))
            .arg(ArgSpec::float("level", "Level"));

        assert!(parse(&spec, &words("on 0.5")).is_ok());
        assert!(parse(&spec, &words("maybe 0.5")).is_err());
        assert!(parse(&spec, &words("on NaN")).is_err());
    }

    #[test]
    fn validate_rejects_specs_parse_cannot_handle() {
        assert!(validate(&volume()).is_ok());

        let required_after_optional = ActionSpec::new("bad", "Bad")
            .arg(ArgSpec::string("a", "A").optional())
            .arg(ArgSpec::string("b", "B"));
        assert!(validate(&required_after_optional).is_err());

        let rest_not_last = ActionSpec::new("bad", "Bad")
            .arg(ArgSpec::string("a", "A").rest())
            .arg(ArgSpec::string("b", "B"));
        assert!(validate(&rest_not_last).is_err());

        let duplicate = ActionSpec::new("bad", "Bad")
            .arg(ArgSpec::string("a", "A"))
            .arg(ArgSpec::int("a", "A again"));
        assert!(validate(&duplicate).is_err());
    }
}

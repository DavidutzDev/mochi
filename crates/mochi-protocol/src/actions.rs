//! Descriptions of the actions a module accepts. The daemon validates
//! commands against them, and `mochi ipc list` prints them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleActions {
    pub module: String,
    pub actions: Vec<ActionSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionSpec {
    pub name: String,
    pub help: String,
    #[serde(default)]
    pub args: Vec<ArgSpec>,
}

impl ActionSpec {
    pub fn new(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            help: help.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: ArgSpec) -> Self {
        self.args.push(arg);
        self
    }

    /// One line such as `show <view> [text...]`.
    pub fn usage(&self) -> String {
        let mut line = self.name.clone();
        for arg in &self.args {
            let dots = if arg.rest { "..." } else { "" };
            let word = if arg.optional {
                format!(" [{}{dots}]", arg.name)
            } else {
                format!(" <{}{dots}>", arg.name)
            };
            line.push_str(&word);
        }
        line
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgSpec {
    pub name: String,
    pub help: String,
    pub kind: ArgKind,
    /// May be left out. Only trailing arguments can be optional.
    #[serde(default)]
    pub optional: bool,
    /// Takes every remaining word, joined with spaces. Only the last argument
    /// can be `rest`.
    #[serde(default)]
    pub rest: bool,
}

impl ArgSpec {
    fn new(name: impl Into<String>, help: impl Into<String>, kind: ArgKind) -> Self {
        Self {
            name: name.into(),
            help: help.into(),
            kind,
            optional: false,
            rest: false,
        }
    }

    pub fn string(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self::new(name, help, ArgKind::String)
    }

    pub fn int(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self::new(name, help, ArgKind::Int)
    }

    pub fn float(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self::new(name, help, ArgKind::Float)
    }

    pub fn bool(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self::new(name, help, ArgKind::Bool)
    }

    pub fn choice<I, S>(name: impl Into<String>, help: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let values = values.into_iter().map(Into::into).collect();
        Self::new(name, help, ArgKind::Choice { values })
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub fn rest(mut self) -> Self {
        self.rest = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArgKind {
    String,
    /// A signed integer. A leading `+` is allowed, as in `volume +5`.
    Int,
    Float,
    /// `true`/`false`, `on`/`off` or `yes`/`no`.
    Bool,
    /// One of a fixed set of words.
    Choice {
        values: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn usage_marks_optional_and_rest_arguments() {
        let spec = ActionSpec::new("show", "Show a view")
            .arg(ArgSpec::string("view", "Which view"))
            .arg(ArgSpec::string("text", "Body").optional().rest());
        assert_eq!(spec.usage(), "show <view> [text...]");
    }

    #[test]
    fn builder_produces_the_listed_shape() {
        let spec = ActionSpec::new("dnd", "Do not disturb")
            .arg(ArgSpec::choice("state", "New state", ["on", "off", "toggle"]).optional());

        assert_eq!(
            serde_json::to_value(&spec).unwrap(),
            json!({
                "name": "dnd",
                "help": "Do not disturb",
                "args": [{
                    "name": "state",
                    "help": "New state",
                    "kind": { "type": "choice", "values": ["on", "off", "toggle"] },
                    "optional": true,
                    "rest": false
                }]
            })
        );
    }
}

//! `mochi-plugin.toml`: what a plugin is, what it runs and what it offers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mochi_protocol::{API, ActionSpec, ArgKind, ArgSpec, Contribution};
use serde::Deserialize;

/// The manifest's file name, at the root of the plugin.
pub const FILE: &str = "mochi-plugin.toml";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub plugin: Info,
    /// Without one, the plugin is only views.
    #[serde(default)]
    pub backend: Option<Backend>,
    /// For `git-release:` sources.
    #[serde(default)]
    pub release: Option<Release>,
    #[serde(default)]
    pub views: Views,
    #[serde(default)]
    pub uses: Uses,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub contributions: Vec<ManifestContribution>,
    /// What the settings panel can't tell from `settings.toml` alone, by
    /// key: `[settings.units]`, or `[settings."cpu.notice"]` in a table.
    #[serde(default)]
    pub settings: BTreeMap<String, SettingHint>,
}

/// How the settings panel shows one option. Its kind and default come
/// from its `# key = value` line in `settings.toml`, its description from
/// the comment above that line.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingHint {
    /// The values it takes, shown as a choice.
    #[serde(default)]
    pub choices: Vec<String>,
    /// A number's range: both give a slider.
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// A `#rrggbb` or `#aarrggbb` color, with a picker.
    #[serde(default)]
    pub color: bool,
    /// It may be left unset.
    #[serde(default)]
    pub optional: bool,
    /// In place of the comment in `settings.toml`.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Info {
    /// Its name in config.toml and `mochi ipc`.
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// The protocol version the backend speaks.
    pub api: u32,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backend {
    /// The program, relative to the plugin's directory.
    pub exec: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// A shell command that builds `exec` from source, run in the plugin's
    /// directory by `mochi plugins install`.
    #[serde(default)]
    pub build: Option<String>,
    /// How Nix builds it, when the plugin's files don't tell.
    #[serde(default)]
    pub kind: Option<Kind>,
    /// Programs the backend runs, by command name, like `python3`: Nix puts
    /// them on its PATH, and elsewhere Mochi warns when one is missing.
    #[serde(default)]
    pub needs: Vec<String>,
}

/// How Nix builds a backend, after the lock file it follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Rust,
    Node,
    Python,
    Go,
    /// As it is: a script, or a binary from a release.
    Files,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    /// The release asset's file name, with `{id}`, `{version}`, `{tag}` and
    /// `{arch}` (like `x86_64`) filled in: a tar archive holding the
    /// manifest, the views and the built `exec`.
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Views {
    /// The QML directory, relative to the plugin's.
    #[serde(default = "Views::default_dir")]
    pub dir: String,
    /// Builtin views it replaces, as `<module>/<view>`: the file
    /// `overrides/<module>/<view>.qml` in its views takes their place.
    #[serde(default)]
    pub overrides: Vec<String>,
}

impl Views {
    fn default_dir() -> String {
        "qml".into()
    }
}

impl Default for Views {
    fn default() -> Self {
        Self {
            dir: Self::default_dir(),
            overrides: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Uses {
    /// Modules whose published state the backend receives.
    #[serde(default)]
    pub state: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub args: Vec<Arg>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arg {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// `string`, `int`, `float`, `bool` or `choice`.
    #[serde(default = "Arg::default_kind")]
    pub kind: String,
    /// The words a `choice` takes.
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub optional: bool,
    /// Takes every remaining word.
    #[serde(default)]
    pub rest: bool,
}

impl Arg {
    fn default_kind() -> String {
        "string".into()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestContribution {
    pub target: String,
    pub kind: String,
    pub id: String,
    /// Empty for kinds without one, like a launcher provider.
    #[serde(default)]
    pub view: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub options: Option<toml::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("no {FILE} in {0}")]
    Missing(PathBuf),
    #[error("cannot read {path}: {error}")]
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("{path}: {message}")]
    Invalid { path: PathBuf, message: String },
}

impl Manifest {
    /// The commands in the backend's `needs` that aren't on the PATH.
    pub fn missing_needs(&self) -> Vec<&str> {
        self.backend
            .iter()
            .flat_map(|backend| &backend.needs)
            .filter(|command| !crate::on_path(command))
            .map(String::as_str)
            .collect()
    }

    /// Reads and checks the manifest in a plugin's directory.
    pub fn load(dir: &Path) -> Result<Self, ManifestError> {
        let path = dir.join(FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ManifestError::Missing(dir.to_owned()));
            }
            Err(error) => return Err(ManifestError::Read { path, error }),
        };
        Self::parse(&text).map_err(|message| ManifestError::Invalid { path, message })
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let manifest: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        manifest.check()?;
        Ok(manifest)
    }

    fn check(&self) -> Result<(), String> {
        check_id(&self.plugin.id)?;
        if self.plugin.api != API {
            return Err(format!(
                "it speaks api {}, this Mochi speaks api {API}",
                self.plugin.api
            ));
        }
        for action in &self.actions {
            for arg in &action.args {
                kind(arg).map_err(|error| format!("action {}: {error}", action.name))?;
            }
        }
        for name in &self.views.overrides {
            if split_override(name).is_none() {
                return Err(format!(
                    "override {name:?} should be <module>/<view>, like \"idle/Pill\""
                ));
            }
        }
        if let Some(backend) = &self.backend {
            if backend.exec.is_empty() || Path::new(&backend.exec).is_absolute() {
                return Err("backend.exec should be a path inside the plugin".into());
            }
            if let Some(command) = backend
                .needs
                .iter()
                .find(|command| command.is_empty() || command.contains(['/', ' ']))
            {
                return Err(format!(
                    "backend.needs takes command names, like \"python3\", not {command:?}"
                ));
            }
        }
        Ok(())
    }

    /// The actions `mochi ipc <id>` accepts.
    pub fn actions(&self) -> Vec<ActionSpec> {
        self.actions
            .iter()
            .map(|action| ActionSpec {
                name: action.name.clone(),
                help: action.description.clone(),
                args: action
                    .args
                    .iter()
                    .map(|arg| ArgSpec {
                        name: arg.name.clone(),
                        help: arg.description.clone(),
                        kind: kind(arg).expect("checked on load"),
                        optional: arg.optional,
                        rest: arg.rest,
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn contributions(&self) -> Vec<Contribution> {
        self.contributions
            .iter()
            .map(|offer| Contribution {
                module: self.plugin.id.clone(),
                target: offer.target.clone(),
                kind: offer.kind.clone(),
                id: offer.id.clone(),
                view: offer.view.clone(),
                title: offer.title.clone(),
                icon: offer.icon.clone(),
                order: offer.order,
                options: offer
                    .options
                    .clone()
                    .and_then(|options| serde_json::to_value(options).ok())
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// The overrides as (module, view) pairs.
    pub fn overrides(&self) -> Vec<(String, String)> {
        self.views
            .overrides
            .iter()
            .filter_map(|name| split_override(name))
            .collect()
    }
}

/// The module id of what the daemon itself shows in the island, like the
/// list of hidden bubbles.
pub const CORE_ID: &str = "mochi";

/// Ids are what config.toml and `mochi ipc` use, and a directory name.
pub fn check_id(id: &str) -> Result<(), String> {
    if id == CORE_ID {
        return Err(format!("plugin id {id:?} is taken by mochi itself"));
    }
    let fine = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && id.starts_with(|c: char| c.is_ascii_lowercase());
    if fine {
        Ok(())
    } else {
        Err(format!(
            "plugin id {id:?} should be lowercase letters, digits, - and _, starting with a letter"
        ))
    }
}

fn split_override(name: &str) -> Option<(String, String)> {
    let (module, view) = name.split_once('/')?;
    let plain = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    (plain(module) && plain(view)).then(|| (module.to_owned(), view.to_owned()))
}

fn kind(arg: &Arg) -> Result<ArgKind, String> {
    Ok(match arg.kind.as_str() {
        "string" => ArgKind::String,
        "int" => ArgKind::Int,
        "float" => ArgKind::Float,
        "bool" => ArgKind::Bool,
        "choice" if !arg.choices.is_empty() => ArgKind::Choice {
            values: arg.choices.clone(),
        },
        "choice" => return Err(format!("<{}> is a choice without choices", arg.name)),
        other => {
            return Err(format!(
                "<{}> has kind {other:?}; use string, int, float, bool or choice",
                arg.name
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const POMODORO: &str = r#"
[plugin]
id = "pomodoro"
name = "Pomodoro"
version = "0.1.0"
description = "A focus timer on the island"
api = 1

[backend]
exec = "bin/pomodoro"
build = "cargo build --release"

[release]
asset = "pomodoro-{arch}-linux.tar.gz"

[views]
overrides = ["idle/Pill"]

[uses]
state = ["media"]

[[actions]]
name = "start"
description = "Start a focus session"
args = [{ name = "minutes", kind = "int", optional = true }]

[[actions]]
name = "mode"
description = "Pick a mode"
args = [{ name = "mode", kind = "choice", choices = ["focus", "break"] }]

[[contributions]]
target = "hub"
kind = "card"
id = "timer"
view = "Card"
title = "Focus"
options = { span = 2 }
"#;

    #[test]
    fn a_full_manifest_parses() {
        let manifest = Manifest::parse(POMODORO).unwrap();
        assert_eq!(manifest.plugin.id, "pomodoro");
        assert_eq!(manifest.views.dir, "qml");
        assert_eq!(
            manifest.overrides(),
            vec![("idle".to_owned(), "Pill".to_owned())]
        );

        let actions = manifest.actions();
        assert_eq!(actions[0].usage(), "start [minutes]");
        assert_eq!(actions[0].args[0].kind, ArgKind::Int);
        assert!(
            matches!(&actions[1].args[0].kind, ArgKind::Choice { values } if values.len() == 2)
        );

        let contributions = manifest.contributions();
        assert_eq!(contributions[0].module, "pomodoro");
        assert_eq!(contributions[0].options, json!({ "span": 2 }));
    }

    #[test]
    fn views_only_is_enough() {
        let manifest = Manifest::parse(
            "[plugin]\nid = \"clock\"\nname = \"Clock\"\nversion = \"1\"\napi = 1\n",
        )
        .unwrap();
        assert!(manifest.backend.is_none());
        assert!(manifest.actions().is_empty());
    }

    #[test]
    fn mistakes_are_named() {
        let error = |text: &str| Manifest::parse(text).unwrap_err();
        let base = "[plugin]\nname = \"X\"\nversion = \"1\"\n";

        assert!(error(&format!("{base}id = \"Bad Id\"\napi = 1\n")).contains("lowercase"));
        assert!(error(&format!("{base}id = \"mochi\"\napi = 1\n")).contains("taken"));
        assert!(error(&format!("{base}id = \"x\"\napi = 9\n")).contains("api 9"));
        assert!(error(&format!("{base}id = \"x\"\napi = 1\nnope = 1\n")).contains("nope"));
        assert!(
            error(&format!(
                "{base}id = \"x\"\napi = 1\n[views]\noverrides = [\"Pill\"]\n"
            ))
            .contains("<module>/<view>")
        );
        assert!(
            error(&format!(
                "{base}id = \"x\"\napi = 1\n[[actions]]\nname = \"a\"\ndescription = \"\"\nargs = [{{ name = \"n\", kind = \"number\" }}]\n"
            ))
            .contains("kind \"number\"")
        );
        assert!(
            error(&format!(
                "{base}id = \"x\"\napi = 1\n[backend]\nexec = \"/usr/bin/x\"\n"
            ))
            .contains("inside the plugin")
        );
    }
}

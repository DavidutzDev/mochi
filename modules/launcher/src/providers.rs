//! Where the launcher's results come from. Each provider answers queries:
//! the built-in `apps`, `calculator` and `commands`, scripts from
//! `config.toml`, and providers other modules and plugins offer.
//!
//! A provider with a prefix answers only queries that start with it, and
//! gets the rest. The others answer every query, and their results show
//! together, in the providers' order. The calculator also answers a query
//! that is plainly math, like `2+2`, without its prefix.
//!
//! Scripts and plugins answer with JSON lines, one result each:
//!
//! ```json
//! {"title": "4", "subtitle": "2+2", "icon": "accessories-calculator", "copy": "4"}
//! ```
//!
//! `title` is required. `glyph` is a short text shown big in place of the
//! icon, like an emoji. At most one verb says what Enter does: `copy`
//! text, `type` it into the window the user was in, `open` a URL or file,
//! or `run` a shell command. `id`, with or without a verb, goes back to the
//! provider's `pick` once the user picks the result.

use std::collections::BTreeMap;

use mochi_core::Contribution;
use serde::Deserialize;
use serde_json::Value;

/// How long a script gets to answer before it is stopped.
pub const DEFAULT_TIMEOUT_MS: u64 = 2000;

/// `[module.launcher.engines.<bang>]`: a web search, asked with `!<bang>`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Engine {
    pub title: String,
    /// `%s` is where the query goes. Empty removes the engine.
    pub url: String,
}

/// The engines there are without any set, by bang.
const ENGINES: [(&str, &str, &str); 6] = [
    ("w", "DuckDuckGo", "https://duckduckgo.com/?q=%s"),
    ("g", "Google", "https://www.google.com/search?q=%s"),
    (
        "gh",
        "GitHub",
        "https://github.com/search?q=%s&type=repositories",
    ),
    (
        "yt",
        "YouTube",
        "https://www.youtube.com/results?search_query=%s",
    ),
    (
        "nix",
        "NixOS packages",
        "https://search.nixos.org/packages?query=%s",
    ),
    (
        "wiki",
        "Wikipedia",
        "https://en.wikipedia.org/w/index.php?search=%s",
    ),
];

/// The default engines with the user's on top: a bang they name replaces
/// the default's, an empty `url` removes it.
pub fn engines(user: &BTreeMap<String, Engine>) -> BTreeMap<String, Engine> {
    let mut engines: BTreeMap<String, Engine> = ENGINES
        .iter()
        .map(|(bang, title, url)| {
            let engine = Engine {
                title: (*title).to_owned(),
                url: (*url).to_owned(),
            };
            ((*bang).to_owned(), engine)
        })
        .collect();
    for (bang, engine) in user {
        let mut engine = engine.clone();
        if engine.title.is_empty() {
            engine.title = engines
                .get(bang)
                .map_or_else(|| bang.clone(), |known| known.title.clone());
        }
        engines.insert(bang.clone(), engine);
    }
    engines.retain(|_, engine| !engine.url.is_empty());
    engines
}

/// `url` with the query in place of `%s`: every byte but letters, digits
/// and `-._~` as `%XX`.
pub fn web_url(url: &str, query: &str) -> String {
    let mut encoded = String::new();
    for byte in query.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    url.replace("%s", &encoded)
}

/// `[module.launcher.providers.<name>]`: changes a provider, or adds a
/// script one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderSettings {
    /// `false` turns it off.
    pub enabled: Option<bool>,
    /// Replaces its prefix; `""` asks it on every query.
    pub prefix: Option<String>,
    /// Lower shows first. Apps are at 0.
    pub order: Option<i32>,
    /// A script provider: the program and its arguments. The query comes
    /// after them, and in `MOCHI_QUERY`.
    pub command: Vec<String>,
    /// Run with a result's `id` after the user picks it.
    pub pick: Vec<String>,
    /// The section heading over its results.
    pub title: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Apps,
    Calculator,
    Commands,
    /// Files and folders from the index.
    Files,
    /// Open windows, on every workspace.
    Windows,
    /// A search engine: `url` with `%s` where the query goes.
    Web {
        url: String,
    },
    Script {
        command: Vec<String>,
        pick: Vec<String>,
        timeout_ms: u64,
    },
    /// A module's provider: its `search` action answers queries.
    Module {
        module: String,
        search: String,
        pick: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    pub name: String,
    pub title: String,
    pub prefix: Option<String>,
    pub order: i32,
    pub kind: Kind,
}

/// Every provider, in order: the built-ins, the scripts in `settings`, and
/// the ones in `offers`, with the user's changes applied.
pub fn providers(
    settings: &BTreeMap<String, ProviderSettings>,
    engines: &BTreeMap<String, Engine>,
    offers: &[Contribution],
) -> Vec<Provider> {
    let builtin = |name: &str, title: &str, prefix: Option<&str>, order, kind| Provider {
        name: name.to_owned(),
        title: title.to_owned(),
        prefix: prefix.map(str::to_owned),
        order,
        kind,
    };
    let mut providers = vec![
        builtin("calculator", "Calculator", Some("="), -10, Kind::Calculator),
        builtin("apps", "Apps", None, 0, Kind::Apps),
        builtin("windows", "Windows", None, -5, Kind::Windows),
        builtin("commands", "Run", Some(">"), 10, Kind::Commands),
        builtin("files", "Files", Some("/"), 15, Kind::Files),
    ];
    for (bang, engine) in engines {
        if engine.url.is_empty() {
            continue;
        }
        providers.push(Provider {
            name: format!("web-{bang}"),
            title: engine.title.clone(),
            prefix: Some(format!("!{bang}")),
            order: 16,
            kind: Kind::Web {
                url: engine.url.clone(),
            },
        });
    }
    for (name, script) in settings {
        if script.command.is_empty() {
            continue;
        }
        if providers.iter().any(|provider| &provider.name == name) {
            tracing::warn!(provider = %name, "a script provider can't take a built-in provider's name");
            continue;
        }
        providers.push(Provider {
            name: name.clone(),
            title: script.title.clone().unwrap_or_else(|| name.clone()),
            prefix: None,
            order: 20,
            kind: Kind::Script {
                command: script.command.clone(),
                pick: script.pick.clone(),
                timeout_ms: script.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
            },
        });
    }
    for offer in offers.iter().filter(|offer| offer.kind == "provider") {
        let option = |key: &str| offer.options.get(key).and_then(Value::as_str);
        if providers.iter().any(|provider| provider.name == offer.id) {
            tracing::warn!(module = %offer.module, provider = %offer.id, "a provider with this name exists already");
            continue;
        }
        providers.push(Provider {
            name: offer.id.clone(),
            title: offer.title.clone(),
            prefix: option("prefix").map(str::to_owned),
            order: offer.order,
            kind: Kind::Module {
                module: offer.module.clone(),
                search: option("search").unwrap_or("search").to_owned(),
                pick: option("pick").map(str::to_owned),
            },
        });
    }

    providers.retain_mut(|provider| {
        let Some(changes) = settings.get(&provider.name) else {
            return true;
        };
        if let Some(prefix) = &changes.prefix {
            provider.prefix = Some(prefix.clone()).filter(|prefix| !prefix.is_empty());
        }
        if let Some(order) = changes.order {
            provider.order = order;
        }
        if let Some(title) = &changes.title {
            provider.title.clone_from(title);
        }
        changes.enabled != Some(false)
    });
    // Stable, so equal orders keep the order above.
    providers.sort_by_key(|provider| provider.order);
    providers
}

/// Which providers answer `query`, and what each gets: the one whose prefix
/// it starts with, the longest when several do, with the rest; otherwise
/// every provider without a prefix, with all of it.
pub fn route<'a>(providers: &'a [Provider], query: &str) -> Vec<(&'a Provider, String)> {
    let prefixed = providers
        .iter()
        .filter_map(|provider| {
            let prefix = provider.prefix.as_deref()?;
            query
                .strip_prefix(prefix)
                .map(|rest| (provider, prefix.len(), rest))
        })
        .max_by_key(|(_, length, _)| *length);
    if let Some((provider, _, rest)) = prefixed {
        return vec![(provider, rest.trim_start().to_owned())];
    }
    providers
        .iter()
        .filter(|provider| provider.prefix.is_none() || provider.kind == Kind::Calculator)
        .map(|provider| (provider, query.to_owned()))
        .collect()
}

/// What Enter does with a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verb {
    /// Starts an app: its desktop id, or `id:action`.
    Launch(String),
    Copy(String),
    Type(String),
    Open(String),
    Run(String),
    /// A command, in the terminal from the settings.
    RunInTerminal(String),
    /// Focuses an open window, by its compositor id.
    Focus(u32),
    /// Shows a file selected in its folder, in the file manager.
    Show(String),
}

/// One line in the list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Item {
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub glyph: Option<String>,
    /// A CSS color, shown as a swatch in place of the icon.
    pub color: Option<String>,
    pub verb: Option<Verb>,
    /// What Shift+Enter does instead.
    pub alt: Option<Verb>,
    /// For the provider's `pick`.
    pub id: Option<String>,
    /// An app's action, drawn a step in.
    pub small: bool,
    /// A file or folder the result is: it drags out onto other apps, and
    /// its folder button shows it in the file manager.
    pub file: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    title: String,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    glyph: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    copy: Option<String>,
    #[serde(default, rename = "type")]
    type_: Option<String>,
    #[serde(default)]
    open: Option<String>,
    #[serde(default)]
    run: Option<String>,
    #[serde(default)]
    alt: Option<Verbs>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    file: Option<String>,
}

/// At most one of them.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Verbs {
    #[serde(default)]
    copy: Option<String>,
    #[serde(default, rename = "type")]
    type_: Option<String>,
    #[serde(default)]
    open: Option<String>,
    #[serde(default)]
    run: Option<String>,
}

impl Verbs {
    fn verb(self) -> Result<Option<Verb>, String> {
        let verbs = [
            self.copy.map(Verb::Copy),
            self.type_.map(Verb::Type),
            self.open.map(Verb::Open),
            self.run.map(Verb::Run),
        ];
        let mut verbs = verbs.into_iter().flatten();
        let verb = verbs.next();
        if verbs.next().is_some() {
            return Err("more than one of copy, type, open and run".into());
        }
        Ok(verb)
    }
}

/// A provider's answer as results. Lines that aren't a result are left out
/// and counted in the error, so a broken provider says so in the log.
pub fn parse(output: &str, limit: usize) -> (Vec<Item>, Option<String>) {
    let mut items = Vec::new();
    let mut bad = Vec::new();
    for (number, line) in output.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match item(line) {
            Ok(item) => items.push(item),
            Err(error) => bad.push(format!("line {}: {error}", number + 1)),
        }
    }
    items.truncate(limit);
    let error = (!bad.is_empty()).then(|| bad.join("; "));
    (items, error)
}

fn item(line: &str) -> Result<Item, String> {
    let line: Line = serde_json::from_str(line).map_err(|error| error.to_string())?;
    // A command runs in a terminal on Shift+Enter, unless it says otherwise.
    let verb = Verbs {
        copy: line.copy,
        type_: line.type_,
        open: line.open,
        run: line.run,
    }
    .verb()?;
    let alt = match line.alt {
        Some(alt) => alt.verb()?,
        None => match &verb {
            Some(Verb::Run(command)) => Some(Verb::RunInTerminal(command.clone())),
            // A file shows in its folder.
            _ => line.file.clone().map(Verb::Show),
        },
    };
    Ok(Item {
        title: line.title,
        subtitle: line.subtitle,
        icon: line.icon,
        glyph: line.glyph,
        color: line.color,
        verb,
        alt,
        id: line.id,
        small: false,
        file: line.file,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn offer(id: &str, options: Value) -> Contribution {
        Contribution {
            module: "emoji".into(),
            target: "launcher".into(),
            kind: "provider".into(),
            id: id.into(),
            view: String::new(),
            title: "Emoji".into(),
            icon: None,
            order: 30,
            options,
        }
    }

    fn names(providers: &[Provider]) -> Vec<&str> {
        providers
            .iter()
            .map(|provider| provider.name.as_str())
            .collect()
    }

    #[test]
    fn built_ins_scripts_and_offers_in_order() {
        let mut settings = BTreeMap::new();
        settings.insert(
            "web".to_owned(),
            ProviderSettings {
                prefix: Some("!w".into()),
                command: vec!["web-search".into()],
                ..ProviderSettings::default()
            },
        );
        settings.insert(
            "commands".to_owned(),
            ProviderSettings {
                enabled: Some(false),
                ..ProviderSettings::default()
            },
        );
        settings.insert(
            "calculator".to_owned(),
            ProviderSettings {
                prefix: Some(String::new()),
                order: Some(5),
                ..ProviderSettings::default()
            },
        );
        let offers = [
            offer("emoji", json!({ "prefix": ":", "pick": "pick" })),
            offer("apps", json!({})),
        ];
        let providers = providers(&settings, &BTreeMap::new(), &offers);
        assert_eq!(
            names(&providers),
            ["windows", "apps", "calculator", "files", "web", "emoji"]
        );
        let web = &providers[4];
        assert_eq!(web.prefix.as_deref(), Some("!w"));
        assert_eq!(web.title, "web");
        assert_eq!(providers[2].prefix, None);
        assert_eq!(
            providers[5].kind,
            Kind::Module {
                module: "emoji".into(),
                search: "search".into(),
                pick: Some("pick".into()),
            }
        );
    }

    #[test]
    fn a_prefix_takes_the_query_and_the_longest_wins() {
        let mut settings = BTreeMap::new();
        for (name, prefix) in [("web", "!w"), ("wiki", "!wiki")] {
            settings.insert(
                name.to_owned(),
                ProviderSettings {
                    prefix: Some(prefix.into()),
                    command: vec!["x".into()],
                    ..ProviderSettings::default()
                },
            );
        }
        let providers = providers(&settings, &BTreeMap::new(), &[]);
        let routed = |query| {
            route(&providers, query)
                .into_iter()
                .map(|(provider, rest)| (provider.name.clone(), rest))
                .collect::<Vec<_>>()
        };
        assert_eq!(routed("=2+2"), [("calculator".into(), "2+2".into())]);
        assert_eq!(routed("> htop"), [("commands".into(), "htop".into())]);
        assert_eq!(routed("!wiki rust"), [("wiki".into(), "rust".into())]);
        assert_eq!(routed("!w rust"), [("web".into(), "rust".into())]);
        // No prefix: apps, open windows, and the calculator for plain math.
        assert_eq!(
            routed("fire"),
            [
                ("calculator".into(), "fire".into()),
                ("windows".into(), "fire".into()),
                ("apps".into(), "fire".into())
            ]
        );
    }

    #[test]
    fn reads_results_and_says_what_it_skipped() {
        let output = concat!(
            r#"{"title": "4", "subtitle": "2+2", "copy": "4"}"#,
            "\n\n",
            r#"{"title": "😀", "glyph": "😀", "id": "grin"}"#,
            "\n",
            "not json\n",
            r#"{"title": "two", "copy": "a", "run": "b"}"#,
            "\n",
            r#"{"title": "rust", "open": "https://www.rust-lang.org"}"#,
        );
        let (items, error) = parse(output, 10);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].verb, Some(Verb::Copy("4".into())));
        assert_eq!(items[1].verb, None);
        assert_eq!(items[1].id.as_deref(), Some("grin"));
        assert_eq!(
            items[2].verb,
            Some(Verb::Open("https://www.rust-lang.org".into()))
        );
        let error = error.unwrap();
        assert!(
            error.contains("line 4") && error.contains("line 5"),
            "{error}"
        );

        let (items, error) = parse(&r#"{"title": "x"}"#.repeat(3).replace("}{", "}\n{"), 2);
        assert_eq!((items.len(), error), (2, None));
    }

    #[test]
    fn engines_have_defaults_the_user_changes() {
        let mut user = BTreeMap::new();
        user.insert(
            "w".to_owned(),
            Engine {
                title: String::new(),
                url: "https://example.org/?q=%s".into(),
            },
        );
        user.insert("g".to_owned(), Engine::default());
        user.insert(
            "crates".to_owned(),
            Engine {
                title: "crates.io".into(),
                url: "https://crates.io/search?q=%s".into(),
            },
        );
        let engines = engines(&user);
        assert_eq!(engines["w"].title, "DuckDuckGo");
        assert_eq!(engines["w"].url, "https://example.org/?q=%s");
        assert!(!engines.contains_key("g"));
        assert_eq!(engines["crates"].title, "crates.io");
        assert!(engines.contains_key("nix"));

        let providers = providers(&BTreeMap::new(), &engines, &[]);
        let routed = route(&providers, "!wiki rust lang");
        assert_eq!(routed[0].0.name, "web-wiki");
        assert_eq!(routed[0].1, "rust lang");
        assert_eq!(route(&providers, "!w x")[0].0.name, "web-w");
    }

    #[test]
    fn encodes_queries_into_urls() {
        assert_eq!(
            web_url("https://duckduckgo.com/?q=%s", "rust \"lang\" & café"),
            "https://duckduckgo.com/?q=rust%20%22lang%22%20%26%20caf%C3%A9"
        );
    }

    #[test]
    fn reads_shift_enter_and_swatches() {
        let (items, error) = parse(
            concat!(
                r#"{"title": "😀", "type": "😀", "alt": {"copy": "😀"}, "id": "😀"}"#,
                "\n",
                r##"{"title": "#1e1e2e", "color": "#1e1e2e", "copy": "#1e1e2e"}"##,
                "\n",
                r#"{"title": "htop", "run": "htop"}"#,
                "\n",
                r#"{"title": "bad", "alt": {"copy": "a", "open": "b"}}"#,
                "\n",
                r#"{"title": "typo", "cpy": "a"}"#,
            ),
            10,
        );
        assert_eq!(items[0].verb, Some(Verb::Type("😀".into())));
        assert_eq!(items[0].alt, Some(Verb::Copy("😀".into())));
        assert_eq!(items[1].color.as_deref(), Some("#1e1e2e"));
        assert_eq!(items[2].alt, Some(Verb::RunInTerminal("htop".into())));
        let error = error.unwrap();
        assert!(
            error.contains("line 4") && error.contains("line 5"),
            "{error}"
        );
    }
}

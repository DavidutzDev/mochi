//! The settings panel's side of the daemon: `config.toml` and `theme.toml`
//! as read, the panel's changes over them (`changes.toml`), every option's
//! default, and the sections the panel shows.
//!
//! The daemon reads the files through here, at startup and on reload, so
//! the changes apply even when the settings module isn't enabled.

use std::path::{Path, PathBuf};

use mochi_core::changes::{self, Changes, File};
use mochi_core::options::{self, Comments, Field, Group, Kind, Section};
use mochi_core::toml::{self, Table, Value};
use mochi_core::{BubblesConfig, Config, ConfigError, IslandConfig, Module, examples};
use mochi_protocol::{Colors, Layout, Motion, Text, Theme};
use serde_json::{Value as Json, json};

use crate::modules::{self, Catalog};

/// The files' tables, or their defaults.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tables {
    pub config: Table,
    pub theme: Table,
}

impl Tables {
    fn get(&self, file: File) -> &Table {
        match file {
            File::Config => &self.config,
            File::Theme => &self.theme,
        }
    }
}

/// A module the panel can turn on and off.
#[derive(Debug, Clone, PartialEq)]
pub struct Available {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub plugin: bool,
}

#[derive(Debug)]
pub struct Store {
    config_file: PathBuf,
    theme_file: PathBuf,
    changes_file: PathBuf,
    /// `--modules`, which replaces the list in the files and the changes.
    modules: Option<Vec<String>>,
    base: Tables,
    changes: Changes,
    /// The system prefers light colors, for `appearance = "auto"`.
    system_light: bool,
    /// Changes being tried, over `changes`: the settings panel's preview,
    /// or the tour showing a look. Never saved, and gone on reload.
    preview: Changes,
    defaults: Tables,
    sections: Vec<Section>,
    available: Vec<Available>,
    /// Each module's actions, by module, for options that run one.
    actions: Json,
}

/// What the daemon runs with.
#[derive(Debug)]
pub struct Loaded {
    pub config: Config,
    pub theme: Theme,
}

impl Store {
    /// Reads the files and the changes. A file with an error is an error;
    /// changes that no longer fit, say after an update renamed an option,
    /// are set aside with a warning so Mochi still starts.
    pub fn load(
        config_file: &Path,
        modules: Option<Vec<String>>,
    ) -> Result<(Self, Loaded), ConfigError> {
        let catalog = modules::catalog(config_file)?;
        let theme_file = config_file.with_file_name("theme.toml");
        let sections = sections(&catalog);
        let mut store = Self {
            config_file: config_file.to_owned(),
            changes_file: Changes::path(config_file),
            theme_file,
            modules,
            base: Tables::default(),
            changes: Changes::default(),
            system_light: false,
            preview: Changes::default(),
            defaults: complete(defaults(&catalog), &sections),
            sections,
            available: available(&catalog),
            actions: actions(&catalog),
        };
        store.base = Tables {
            config: mochi_core::config::read_table(&store.config_file)?,
            theme: mochi_core::config::read_table(&store.theme_file)?,
        };
        // Without changes first: their errors are only warnings.
        let plain = store.resolve(&Changes::default(), &catalog)?;
        let changes = match Changes::load(&store.changes_file) {
            Ok(changes) => changes,
            Err(error) => {
                tracing::warn!(%error, "ignoring the settings panel's changes");
                return Ok((store, plain));
            }
        };
        let changes = store.pruned(changes);
        match store.resolve(&changes, &catalog) {
            Ok(loaded) => {
                store.keep(changes);
                Ok((store, loaded))
            }
            Err(error) => {
                tracing::warn!(%error, file = %store.changes_file.display(), "ignoring the settings panel's changes");
                Ok((store, plain))
            }
        }
    }

    /// The settings panel has changes over the files.
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Reads the files again, for `mochi reload`. Changes they now say go
    /// away.
    pub fn reload(&mut self) -> Result<Loaded, ConfigError> {
        let (store, loaded) = Self::load(&self.config_file, self.modules.take())?;
        *self = store;
        Ok(loaded)
    }

    /// Checks `changes` over the files, as the daemon would run them.
    fn resolve(&self, changes: &Changes, catalog: &Catalog) -> Result<Loaded, ConfigError> {
        let table = changes::merged(&self.base.config, &changes.config);
        let mut config = Config::from_table(table, &self.config_file)?;
        if let Some(modules) = &self.modules {
            config.modules = modules.clone();
        }
        modules::check_config(&config, catalog, &self.config_file)?;
        let theme = mochi_core::config::theme_from_table(
            changes::merged(&self.base.theme, &changes.theme),
            &self.theme_file,
            self.system_light,
        )?;
        Ok(Loaded { config, theme })
    }

    fn pruned(&self, mut changes: Changes) -> Changes {
        for file in File::ALL {
            changes::prune(
                changes.table_mut(file),
                self.base.get(file),
                self.defaults.get(file),
            );
        }
        changes
    }

    /// Makes `changes` the current ones, and saves them when they differ.
    fn keep(&mut self, changes: Changes) {
        let saved = Changes::load(&self.changes_file).unwrap_or_default();
        if saved != changes
            && let Err(error) = changes.save(&self.changes_file)
        {
            tracing::warn!(%error, file = %self.changes_file.display(), "cannot save the settings panel's changes");
        }
        self.changes = changes;
    }

    /// Runs an op that changes something: checks the result, keeps it and
    /// returns what the daemon now runs with.
    pub fn change(&mut self, op: &Op) -> Result<Loaded, String> {
        let mut changes = self.changes.clone();
        let mut preview = self.preview.clone();
        match op {
            Op::Set { path, value } => {
                let (file, parts, value) = self.value(path, value)?;
                // What's kept wins over what's tried.
                changes::remove(preview.table_mut(file), &parts);
                match value {
                    Some(value) => changes::set(changes.table_mut(file), &parts, value),
                    None => changes::remove(changes.table_mut(file), &parts),
                }
            }
            Op::Reset { path } => {
                self.reset(&mut changes, path)?;
                self.reset(&mut preview, path)?;
            }
            Op::Discard => {
                changes = Changes::default();
                preview = Changes::default();
            }
            Op::Edit { path, text } => {
                let (file, parts) = self.section_path(path)?;
                let table: Table = toml::from_str(text).map_err(|error| error.to_string())?;
                changes::remove(preview.table_mut(file), &parts);
                let target = changes.table_mut(file);
                changes::remove(target, &parts);
                changes::set(target, &parts, Value::Table(table));
            }
            Op::Preview { values, replace } => {
                if *replace {
                    preview = Changes::default();
                }
                for (path, value) in values {
                    let (file, parts, value) = self.value(path, value)?;
                    match value {
                        Some(value) => changes::set(preview.table_mut(file), &parts, value),
                        None => changes::remove(preview.table_mut(file), &parts),
                    }
                }
            }
            Op::Keep => {
                for file in File::ALL {
                    changes::merge(changes.table_mut(file), preview.table(file));
                }
                preview = Changes::default();
            }
            Op::Drop => preview = Changes::default(),
        }
        let changes = self.pruned(changes);
        let preview = self.pruned_over(preview, &changes);
        let catalog = modules::catalog(&self.config_file).map_err(|error| error.to_string())?;
        let loaded = self
            .resolve(&layered(&changes, &preview), &catalog)
            .map_err(|error| match error {
                ConfigError::Invalid { message, .. } => message,
                other => other.to_string(),
            })?;
        self.keep(changes);
        self.preview = preview;
        Ok(loaded)
    }

    /// An option's file, its path in the file, and the value the panel
    /// sent as that option's TOML; `None` unsets it.
    fn value<'a>(
        &self,
        path: &'a str,
        value: &Json,
    ) -> Result<(File, Vec<&'a str>, Option<Value>), String> {
        let field = self.field(path)?;
        let (file, parts) = File::split(path).ok_or("no such option")?;
        Ok((
            file,
            parts,
            options::from_json(value, field.kind, field.items)?,
        ))
    }

    /// The preview without what the files and the changes already say.
    fn pruned_over(&self, mut preview: Changes, changes: &Changes) -> Changes {
        for file in File::ALL {
            let under = changes::merged(self.base.get(file), changes.table(file));
            changes::prune(preview.table_mut(file), &under, self.defaults.get(file));
        }
        preview
    }

    /// Drops the panel's change to an option, or to every option of a
    /// section, so it's back to what the files say: their value, or the
    /// default where they say nothing.
    fn reset(&self, changes: &mut Changes, path: &str) -> Result<(), String> {
        let known = self.sections.iter().any(|section| section.path == path);
        if !known {
            self.field(path)?;
        }
        let (file, parts) = File::split(path).ok_or_else(|| format!("no option {path}"))?;
        changes::remove(changes.table_mut(file), &parts);
        Ok(())
    }

    fn field(&self, path: &str) -> Result<&Field, String> {
        self.sections
            .iter()
            .flat_map(|section| &section.fields)
            .find(|field| field.path == path && field.kind != Kind::Group)
            .ok_or_else(|| format!("no option {path}"))
    }

    fn section_path<'a>(&self, path: &'a str) -> Result<(File, Vec<&'a str>), String> {
        if !self.sections.iter().any(|section| section.path == path) {
            return Err(format!("no section {path}"));
        }
        File::split(path).ok_or_else(|| format!("no section {path}"))
    }

    /// A file as it applies: the defaults, the file over them, the changes
    /// over that, and what's being tried on top.
    fn effective(&self, file: File) -> Table {
        let mut table = self.defaults.get(file).clone();
        let mut layers = self.base.get(file).clone();
        changes::merge(&mut layers, self.changes.table(file));
        changes::merge(&mut layers, self.preview.table(file));
        if file == File::Theme {
            changes::merge(&mut table, &self.palette(&layers));
        }
        changes::merge(&mut table, &layers);
        table
    }

    /// The colors the preset in `theme` gives, as `[colors]`: what the
    /// color options show where nothing sets them.
    fn palette(&self, theme: &Table) -> Table {
        let colors = mochi_core::config::palette_of(theme, self.system_light).unwrap_or_default();
        Table::from_iter([("colors".to_owned(), Value::Table(colors))])
    }

    /// The system's light or dark preference changed, for
    /// `appearance = "auto"`: what the daemon now runs with.
    pub fn set_system_light(&mut self, light: bool) -> Option<Loaded> {
        if self.system_light == light {
            return None;
        }
        self.system_light = light;
        let catalog = modules::catalog(&self.config_file).ok()?;
        self.resolve(&layered(&self.changes, &self.preview), &catalog)
            .ok()
    }

    /// What the panel shows: every section with each field's value.
    pub fn snapshot(&self) -> Json {
        let config = self.effective(File::Config);
        let theme = self.effective(File::Theme);
        // What the files say, without the changes.
        let saved = |file: File| {
            let mut table = self.defaults.get(file).clone();
            if file == File::Theme {
                changes::merge(&mut table, &self.palette(self.base.get(file)));
            }
            changes::merge(&mut table, self.base.get(file));
            table
        };
        // A color's default is the preset's.
        let palette = self.palette(&theme);
        let (saved_config, saved_theme) = (saved(File::Config), saved(File::Theme));
        let enabled: Vec<String> = match changes::get(&config, &["modules"]) {
            Some(Value::Array(ids)) => ids
                .iter()
                .filter_map(|id| id.as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        };
        let sections: Vec<Json> = self
            .sections
            .iter()
            .map(|section| {
                let fields: Vec<Json> = section
                    .fields
                    .iter()
                    .map(|field| {
                        let (file, parts) = File::split(&field.path).expect("a known file");
                        let table = if file == File::Config {
                            &config
                        } else {
                            &theme
                        };
                        let value = changes::get(table, &parts)
                            .map(options::to_json)
                            .unwrap_or(Json::Null);
                        // Changed here, not only different from the default.
                        let changed = changes::get(self.changes.table(file), &parts).is_some();
                        let previewed = changes::get(self.preview.table(file), &parts).is_some();
                        let saved = if file == File::Config {
                            &saved_config
                        } else {
                            &saved_theme
                        };
                        let saved = changes::get(saved, &parts)
                            .map(options::to_json)
                            .unwrap_or(Json::Null);
                        let mut field = serde_json::to_value(field).expect("serializes");
                        if file == File::Theme
                            && let Some(color) = changes::get(&palette, &parts)
                        {
                            field["default"] = options::to_json(color);
                        }
                        if field.get("suggestions").is_some() {
                            field["suggestions"] = installed(&field["suggestions"]);
                        }
                        field["value"] = value;
                        field["saved"] = saved;
                        field["changed"] = Json::Bool(changed || previewed);
                        field["previewed"] = Json::Bool(previewed);
                        field
                    })
                    .collect();
                let mut section = serde_json::to_value(section).expect("serializes");
                section["fields"] = Json::Array(fields);
                if let Some(module) = section.get("module").and_then(Json::as_str) {
                    let on = enabled.iter().any(|id| id == module);
                    section["enabled"] = Json::Bool(on);
                }
                section
            })
            .collect();
        let modules: Vec<Json> = self
            .available
            .iter()
            .map(|module| {
                json!({
                    "id": module.id,
                    "title": module.title,
                    "icon": module.icon,
                    "plugin": module.plugin,
                    "enabled": enabled.contains(&module.id),
                })
            })
            .collect();
        json!({
            "sections": sections,
            "modules": modules,
            "enabled": enabled,
            "changes": !self.changes.is_empty(),
            "previewing": !self.preview.is_empty(),
            "fixed_modules": self.modules.is_some(),
            "actions": self.actions,
        })
    }

    /// A section as TOML, every option in it at its value.
    pub fn text(&self, path: &str) -> Result<String, String> {
        let (file, parts) = self.section_path(path)?;
        let table = self.effective(file);
        let section = changes::get(&table, &parts)
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_default();
        toml::to_string_pretty(&section).map_err(|error| error.to_string())
    }

    /// Every option that isn't at its default, as Nix for `programs.mochi`
    /// or as the two TOML files.
    pub fn export(&self, format: &str) -> Result<String, String> {
        let mut config = changes::merged(&self.base.config, &self.changes.config);
        let theme = changes::merged(&self.base.theme, &self.changes.theme);
        // The list is always worth writing down: it says what runs.
        let modules = config.remove("modules");
        let mut config = changes::diff(&config, &self.defaults.config);
        if let Some(modules) = modules {
            config.insert("modules".to_owned(), modules);
        }
        let theme = changes::diff(&theme, &self.defaults.theme);
        match format {
            "nix" => {
                let mut out = String::new();
                for (name, table) in [("settings", config), ("theme", theme)] {
                    if !table.is_empty() {
                        out.push_str(&binding(name, table));
                    }
                }
                Ok(out)
            }
            "toml" => {
                let mut parts = Vec::new();
                for (name, table) in [("config.toml", config), ("theme.toml", theme)] {
                    if !table.is_empty() {
                        let text = toml::to_string_pretty(&table).map_err(|e| e.to_string())?;
                        parts.push(format!("# {name}\n{text}"));
                    }
                }
                Ok(parts.join("\n"))
            }
            other => Err(format!("unknown format {other:?}: nix or toml")),
        }
    }
}

/// `preview` laid over `changes`, as one set of changes.
fn layered(changes: &Changes, preview: &Changes) -> Changes {
    let mut out = changes.clone();
    for file in File::ALL {
        changes::merge(out.table_mut(file), preview.table(file));
    }
    out
}

fn binding(name: &str, table: Table) -> String {
    // A whole attribute set, even with one key: it pastes as one block.
    let mut out = format!("{name} = {{\n");
    for (key, value) in &table {
        out.push_str(&mochi_core::nix::binding(key, value, 2));
    }
    out.push_str("};\n");
    out
}

/// What changes the settings: [`SettingsOp`](mochi_core::SettingsOp)
/// without the ops that only read.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Set {
        path: String,
        value: Json,
    },
    Reset {
        path: String,
    },
    Discard,
    Edit {
        path: String,
        text: String,
    },
    /// Tries values without keeping them; `replace` drops what was being
    /// tried first.
    Preview {
        values: Vec<(String, Json)>,
        replace: bool,
    },
    /// Keeps what's being tried, as changes.
    Keep,
    /// Stops trying, back to the changes.
    Drop,
}

/// Every option at its default: the theme's, and for `config.toml` each
/// example's `# key = value` lines.
fn defaults(catalog: &Catalog) -> Tables {
    let mut config = Table::new();
    config.insert(
        "modules".to_owned(),
        Value::Array(vec![Value::String("idle".to_owned())]),
    );
    let examples = [examples::ISLAND, examples::BUBBLES]
        .into_iter()
        .map(str::to_owned)
        .chain(
            catalog
                .modules
                .iter()
                .map(|module| example(catalog, module.as_ref())),
        );
    for example in examples {
        match toml::from_str::<Table>(&examples::uncommented(&example)) {
            Ok(table) => changes::merge(&mut config, &table),
            Err(error) => tracing::warn!(%error, "an example doesn't parse"),
        }
    }
    let theme = Table::try_from(Theme::default()).expect("the theme serializes");
    Tables { config, theme }
}

/// Adds the defaults only the settings types know, like options their
/// example shows as an indented sample instead of a `# key = value` line.
fn complete(mut defaults: Tables, sections: &[Section]) -> Tables {
    for field in sections.iter().flat_map(|section| &section.fields) {
        if field.kind == Kind::Group || field.default.is_null() {
            continue;
        }
        let (file, parts) = File::split(&field.path).expect("a known file");
        let table = match file {
            File::Config => &mut defaults.config,
            File::Theme => &mut defaults.theme,
        };
        if changes::get(table, &parts).is_none()
            && let Ok(Some(value)) = options::from_json(&field.default, field.kind, field.items)
        {
            changes::set(table, &parts, value);
        }
    }
    defaults
}

/// A module's example: a builtin's own, or a plugin's `settings.toml`.
fn example(catalog: &Catalog, module: &dyn Module) -> String {
    match catalog.plugins.get(module.id()) {
        Some(plugin) => plugin.example().to_owned(),
        None => module.settings_example().to_owned(),
    }
}

/// Each module's actions, for options that run one, like the idle clock's
/// click.
fn actions(catalog: &Catalog) -> Json {
    let map: serde_json::Map<String, Json> = catalog
        .modules
        .iter()
        .map(|module| {
            (
                module.id().to_owned(),
                serde_json::to_value(module.actions()).unwrap_or_default(),
            )
        })
        .collect();
    Json::Object(map)
}

/// A field's ready-made commands, the ones whose program is installed.
fn installed(suggestions: &Json) -> Json {
    let kept: Vec<Json> = suggestions
        .as_array()
        .into_iter()
        .flatten()
        .filter(|command| {
            command
                .get(0)
                .and_then(Json::as_str)
                .is_some_and(mochi_core::process::installed)
        })
        .cloned()
        .collect();
    Json::Array(kept)
}

/// The panel's pages, in sidebar order.
fn sections(catalog: &Catalog) -> Vec<Section> {
    let theme_defaults = Table::try_from(Theme::default()).expect("the theme serializes");
    let theme_comments = Comments::parse(examples::THEME);
    let mut out = Vec::new();
    let appearance = [
        (
            "colors",
            "Colors",
            "palette",
            options::schema_of::<Colors>(),
        ),
        ("text", "Text", "text_fields", options::schema_of::<Text>()),
        (
            "layout",
            "Layout",
            "dashboard",
            options::schema_of::<Layout>(),
        ),
        (
            "motion",
            "Motion",
            "animation",
            options::schema_of::<Motion>(),
        ),
    ];
    // The preset, its light or dark version, and the wallpaper go first on
    // the Colors page.
    let mut look: Vec<Field> = options::fields(
        &options::schema_of::<Theme>(),
        "theme",
        "",
        &theme_comments,
        &theme_defaults,
    )
    .into_iter()
    .filter(|field| {
        ["theme.preset", "theme.appearance", "theme.wallpaper"].contains(&field.path.as_str())
    })
    .collect();
    for field in &mut look {
        if field.path == "theme.preset" {
            for choice in &mut field.choices {
                choice.colors = mochi_core::palette::swatches(&choice.value, false);
            }
        }
    }
    for (id, title, icon, schema) in appearance {
        let description = theme_comments
            .heading(id)
            .map(|(_, description)| description)
            .unwrap_or_default();
        out.push(Section {
            id: id.to_owned(),
            path: format!("theme.{id}"),
            title: title.to_owned(),
            description,
            group: Group::Appearance,
            icon: icon.to_owned(),
            module: None,
            fields: if id == "colors" {
                look.drain(..)
                    .chain(options::fields(
                        &schema,
                        "theme",
                        id,
                        &theme_comments,
                        &theme_defaults,
                    ))
                    .collect()
            } else {
                options::fields(&schema, "theme", id, &theme_comments, &theme_defaults)
            },
        });
    }

    let config_defaults = defaults(catalog).config;
    let shell = [
        (
            "island",
            "Island",
            "crop_16_9",
            examples::ISLAND,
            options::schema_of::<IslandConfig>(),
        ),
        (
            "bubbles",
            "Bubbles",
            "bubble_chart",
            examples::BUBBLES,
            options::schema_of::<BubblesConfig>(),
        ),
    ];
    for (id, title, icon, example, schema) in shell {
        let comments = Comments::parse(example);
        let description = comments
            .heading(id)
            .map(|(_, description)| description)
            .unwrap_or_default();
        out.push(Section {
            id: id.to_owned(),
            path: format!("config.{id}"),
            title: title.to_owned(),
            description,
            group: Group::Shell,
            icon: icon.to_owned(),
            module: None,
            fields: options::fields(&schema, "config", id, &comments, &config_defaults),
        });
    }

    // The list of modules, which the panel shows as a switch per module.
    out.push(Section {
        id: "modules".to_owned(),
        path: "config.modules".to_owned(),
        title: "Modules".to_owned(),
        description: "What runs. A module turned off stops at once, and one turned on starts."
            .to_owned(),
        group: Group::Shell,
        icon: "extension".to_owned(),
        module: None,
        fields: vec![Field {
            path: "config.modules".to_owned(),
            title: "Modules".to_owned(),
            description: String::new(),
            kind: Kind::List,
            choices: Vec::new(),
            min: None,
            max: None,
            items: Some(Kind::Text),
            source: Some("module".to_owned()),
            suggestions: Vec::new(),
            optional: false,
            default: json!(mochi_core::config::DEFAULT_MODULES),
        }],
    });

    for module in &catalog.modules {
        let id = module.id();
        let plugin = catalog.plugins.contains_key(id);
        let example = example(catalog, module.as_ref());
        let comments = Comments::parse(&example);
        let table = format!("module.{id}");
        let (title, description) = comments.heading(&table).unwrap_or_default();
        let title = if title.is_empty() {
            options::title(id)
        } else {
            title
        };
        let schema = module.settings_schema().unwrap_or_else(|| json!({}));
        out.push(Section {
            id: id.to_owned(),
            path: format!("config.{table}"),
            title,
            description,
            group: if plugin {
                Group::Plugins
            } else {
                Group::Modules
            },
            icon: icon(id).to_owned(),
            module: Some(id.to_owned()),
            fields: options::fields(&schema, "config", &table, &comments, &config_defaults),
        });
    }
    out
}

fn available(catalog: &Catalog) -> Vec<Available> {
    catalog
        .modules
        .iter()
        .map(|module| {
            let id = module.id();
            let comments = Comments::parse(&example(catalog, module.as_ref()));
            let title = comments
                .heading(&format!("module.{id}"))
                .map(|(title, _)| title)
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| options::title(id));
            Available {
                id: id.to_owned(),
                title,
                icon: icon(id).to_owned(),
                plugin: catalog.plugins.contains_key(id),
            }
        })
        .collect()
}

/// A builtin module's icon in the sidebar.
fn icon(module: &str) -> &'static str {
    match module {
        "audio" => "volume_up",
        "battery" => "battery_full",
        "brightness" => "light_mode",
        "bluetooth" => "bluetooth",
        "capture" => "screenshot_region",
        "clipboard" => "content_paste",
        "colors" => "colorize",
        "drop" => "place_item",
        "emoji" => "mood",
        "hub" => "space_dashboard",
        "idle" => "bedtime",
        "launcher" => "search",
        "media" => "music_note",
        "network" => "wifi",
        "nightlight" => "nightlight",
        "notes" => "sticky_note_2",
        "notifications" => "notifications",
        "osd" => "tune",
        "performance" => "monitor_heart",
        "power" => "power_settings_new",
        "privacy" => "privacy_tip",
        "settings" => "settings",
        "tour" => "tour",
        "share" => "screen_share",
        "tray" => "apps",
        "widgets" => "widgets",
        "workspaces" => "view_carousel",
        _ => "extension",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mochi-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    fn set(path: &str, value: Json) -> Op {
        Op::Set {
            path: path.to_owned(),
            value,
        }
    }

    #[test]
    fn every_source_is_one_the_panel_knows() {
        const KNOWN: [&str; 7] = [
            "command",
            "module",
            "app",
            "audio-device",
            "tray-app",
            "player",
            "hub-card",
        ];
        let catalog = modules::catalog(Path::new("/nonexistent/config.toml")).unwrap();
        let mut found = 0;
        for section in sections(&catalog) {
            for field in &section.fields {
                if let Some(source) = &field.source {
                    assert!(KNOWN.contains(&source.as_str()), "{}: {source}", field.path);
                    found += 1;
                }
            }
        }
        // Idle's two commands, capture's devices, and the rest.
        assert!(found >= 12, "{found}");
    }

    #[test]
    fn every_module_has_a_section_with_fields() {
        let catalog = modules::catalog(Path::new("/nonexistent/config.toml")).unwrap();
        let sections = sections(&catalog);
        for module in &catalog.modules {
            let section = sections
                .iter()
                .find(|section| section.module.as_deref() == Some(module.id()))
                .unwrap_or_else(|| panic!("no section for {}", module.id()));
            if !module.settings_example().contains(" = ") {
                continue;
            }
            assert!(!section.fields.is_empty(), "{} has no fields", module.id());
        }
        let osd = sections.iter().find(|section| section.id == "osd").unwrap();
        assert_eq!(osd.title, "OSD");
        let timeout = osd
            .fields
            .iter()
            .find(|field| field.path == "config.module.osd.timeout_ms")
            .unwrap();
        assert_eq!(timeout.default, json!(1500));
        assert_eq!(timeout.kind, Kind::Int);
        let accent = sections
            .iter()
            .flat_map(|section| &section.fields)
            .find(|field| field.path == "theme.colors.accent")
            .unwrap();
        assert_eq!(accent.kind, Kind::Color);
        let family = sections
            .iter()
            .flat_map(|section| &section.fields)
            .find(|field| field.path == "theme.text.family")
            .unwrap();
        assert_eq!(family.kind, Kind::Font);
    }

    /// Every value of every example is a field, so nothing
    /// the docs show is missing from the panel.
    #[test]
    fn every_example_key_is_a_field() {
        let catalog = modules::catalog(Path::new("/nonexistent/config.toml")).unwrap();
        let sections = sections(&catalog);
        let paths: Vec<&str> = sections
            .iter()
            .flat_map(|section| &section.fields)
            .map(|field| field.path.as_str())
            .collect();
        let defaults = defaults(&catalog);
        for (file, table) in [("config", &defaults.config), ("theme", &defaults.theme)] {
            let mut missing = Vec::new();
            walk(table, file, &mut |path| {
                if path != "config.modules" && !paths.contains(&path) {
                    missing.push(path.to_owned());
                }
            });
            assert!(missing.is_empty(), "no field for {missing:?}");
        }
    }

    /// schemars puts an enum's undocumented variants ahead of its documented
    /// ones, so the panel would list them out of order: document all of an
    /// enum's variants, or none.
    #[test]
    fn enums_document_all_their_variants_or_none() {
        fn mixed(schema: &Json, path: &str, found: &mut Vec<String>) {
            match schema {
                Json::Object(map) => {
                    if let Some(Json::Array(branches)) = map.get("oneOf") {
                        let grouped = branches.iter().any(|branch| branch.get("enum").is_some());
                        let single = branches.iter().any(|branch| branch.get("const").is_some());
                        if grouped && single {
                            found.push(path.to_owned());
                        }
                    }
                    for (key, value) in map {
                        mixed(value, &format!("{path}.{key}"), found);
                    }
                }
                Json::Array(items) => {
                    for item in items {
                        mixed(item, path, found);
                    }
                }
                _ => {}
            }
        }
        let catalog = modules::catalog(Path::new("/nonexistent/config.toml")).unwrap();
        let mut schemas = vec![
            ("theme".to_owned(), options::schema_of::<Theme>()),
            ("island".to_owned(), options::schema_of::<IslandConfig>()),
            ("bubbles".to_owned(), options::schema_of::<BubblesConfig>()),
        ];
        for module in &catalog.modules {
            if let Some(schema) = module.settings_schema() {
                schemas.push((module.id().to_owned(), schema));
            }
        }
        let mut found = Vec::new();
        for (name, schema) in &schemas {
            mixed(schema, name, &mut found);
        }
        assert!(
            found.is_empty(),
            "enums with only some variants documented: {found:?}"
        );
    }

    fn walk(table: &Table, prefix: &str, visit: &mut impl FnMut(&str)) {
        for (key, value) in table {
            let path = format!("{prefix}.{key}");
            match value {
                Value::Table(inner) => walk(inner, &path, visit),
                _ => visit(&path),
            }
        }
    }

    #[test]
    fn changes_apply_save_and_go_once_the_file_says_them() {
        let config = temp("apply");
        std::fs::write(&config, "modules = [\"idle\", \"osd\"]\n").unwrap();
        let (mut store, loaded) = Store::load(&config, None).unwrap();
        assert_eq!(loaded.theme, Theme::default());

        let loaded = store
            .change(&set("theme.colors.accent", json!("#30d158")))
            .unwrap();
        assert_eq!(loaded.theme.colors.accent.as_str(), "#30d158");
        store
            .change(&set("config.module.osd.timeout_ms", json!(2000)))
            .unwrap();
        let saved = Changes::load(&Changes::path(&config)).unwrap();
        assert_eq!(
            changes::get(&saved.config, &["module", "osd", "timeout_ms"]),
            Some(&Value::Integer(2000))
        );

        // A bad value changes nothing.
        let error = store
            .change(&set("theme.motion.damping", json!(3.0)))
            .unwrap_err();
        assert!(error.contains("damping"), "{error}");
        let error = store
            .change(&set("config.module.osd.timeout_ms", json!("soon")))
            .unwrap_err();
        assert!(error.contains("soon"), "{error}");

        // The user pastes the export into their files and reloads.
        let nix = store.export("nix").unwrap();
        assert!(nix.contains("osd.timeout_ms = 2000;"), "{nix}");
        assert!(
            nix.contains("theme = {\n  colors.accent = \"#30d158\";"),
            "{nix}"
        );
        let toml = store.export("toml").unwrap();
        let (config_part, theme_part) = toml.split_once("# theme.toml\n").unwrap();
        std::fs::write(&config, config_part.trim_start_matches("# config.toml\n")).unwrap();
        std::fs::write(config.with_file_name("theme.toml"), theme_part).unwrap();
        store.reload().unwrap();
        assert!(
            !Changes::path(&config).exists(),
            "the changes should be gone"
        );
        assert_eq!(store.snapshot()["changes"], json!(false));
        std::fs::remove_dir_all(config.parent().unwrap()).unwrap();
    }

    #[test]
    fn resetting_goes_back_to_the_file() {
        let config = temp("reset");
        std::fs::write(&config, "[module.osd]\ntimeout_ms = 900\n").unwrap();
        let (mut store, _) = Store::load(&config, None).unwrap();
        let timeout = |store: &Store| {
            store.snapshot()["sections"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|section| section["fields"].as_array().unwrap())
                .find(|field| field["path"] == "config.module.osd.timeout_ms")
                .cloned()
                .unwrap()
        };
        assert_eq!(timeout(&store)["changed"], json!(false));
        store
            .change(&set("config.module.osd.timeout_ms", json!(2000)))
            .unwrap();
        store
            .change(&set("config.module.osd.volume", json!(false)))
            .unwrap();
        assert_eq!(timeout(&store)["changed"], json!(true));
        assert_eq!(timeout(&store)["saved"], json!(900));

        // The file's 900, not the default 1500.
        store
            .change(&Op::Reset {
                path: "config.module.osd.timeout_ms".to_owned(),
            })
            .unwrap();
        assert_eq!(timeout(&store)["value"], json!(900));
        assert_eq!(timeout(&store)["changed"], json!(false));
        assert!(!store.changes.is_empty(), "volume is still changed");

        // A whole section.
        store
            .change(&Op::Reset {
                path: "config.module.osd".to_owned(),
            })
            .unwrap();
        assert!(store.changes.is_empty());
        assert!(
            store
                .change(&Op::Reset {
                    path: "config.module.nope".to_owned()
                })
                .is_err()
        );
        std::fs::remove_dir_all(config.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_preview_applies_without_being_saved() {
        let config = temp("preview");
        let (mut store, _) = Store::load(&config, None).unwrap();
        let tried = |store: &mut Store, values: &[(&str, Json)], replace| {
            store.change(&Op::Preview {
                values: values
                    .iter()
                    .map(|(path, value)| ((*path).to_owned(), value.clone()))
                    .collect(),
                replace,
            })
        };
        let loaded = tried(&mut store, &[("theme.layout.mode", json!("notch"))], false).unwrap();
        assert_eq!(loaded.theme.layout.mode, mochi_protocol::Mode::Notch);
        assert!(!Changes::path(&config).exists(), "a preview is never saved");
        assert_eq!(store.snapshot()["previewing"], json!(true));

        // Replacing drops what was tried before.
        let loaded = tried(
            &mut store,
            &[("theme.colors.accent", json!("#30d158"))],
            true,
        )
        .unwrap();
        assert_eq!(loaded.theme.layout.mode, mochi_protocol::Mode::Island);
        assert_eq!(loaded.theme.colors.accent.as_str(), "#30d158");
        assert!(tried(&mut store, &[("theme.motion.damping", json!(5))], false).is_err());

        // Keeping saves it as a change.
        store.change(&Op::Keep).unwrap();
        assert_eq!(store.snapshot()["previewing"], json!(false));
        let saved = Changes::load(&Changes::path(&config)).unwrap();
        assert_eq!(
            changes::get(&saved.theme, &["colors", "accent"]),
            Some(&Value::String("#30d158".into()))
        );

        // A reload forgets what was tried.
        tried(
            &mut store,
            &[("theme.layout.anchor", json!("bottom"))],
            false,
        )
        .unwrap();
        let loaded = store.reload().unwrap();
        assert_eq!(loaded.theme.layout.anchor, mochi_protocol::Anchor::Top);
        assert_eq!(loaded.theme.colors.accent.as_str(), "#30d158");
        std::fs::remove_dir_all(config.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_section_edits_as_toml() {
        let config = temp("edit");
        let (mut store, _) = Store::load(&config, None).unwrap();
        let text = store.text("config.module.osd").unwrap();
        assert!(text.contains("timeout_ms = 1500"), "{text}");
        store
            .change(&Op::Edit {
                path: "config.module.osd".to_owned(),
                text: text.replace("1500", "2500"),
            })
            .unwrap();
        assert_eq!(
            store.changes.config,
            toml::from_str::<Table>("[module.osd]\ntimeout_ms = 2500").unwrap()
        );
        let error = store
            .change(&Op::Edit {
                path: "config.module.osd".to_owned(),
                text: "timeot_ms = 2".to_owned(),
            })
            .unwrap_err();
        assert!(error.contains("timeot_ms"), "{error}");
        std::fs::remove_dir_all(config.parent().unwrap()).unwrap();
    }

    #[test]
    fn changes_that_no_longer_fit_are_set_aside() {
        let config = temp("stale");
        std::fs::write(
            Changes::path(&config),
            "[config.module.osd]\nrenamed_option = 1\n",
        )
        .unwrap();
        let (store, loaded) = Store::load(&config, None).unwrap();
        assert_eq!(loaded.config, Config::default());
        assert!(store.changes.is_empty());
        std::fs::remove_dir_all(config.parent().unwrap()).unwrap();
    }
}

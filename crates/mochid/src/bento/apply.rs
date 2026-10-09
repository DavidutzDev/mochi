//! Installing what Bento shares, trying it, and taking it out again.
//!
//! A theme goes in `$XDG_DATA_HOME/mochi/themes/<id>/`, a plugin where
//! `mochi plugins` puts them, and both are recorded in `bento.toml`. A
//! bento installs the themes it brings and the plugins it needs, each
//! plugin asking first, then lays its settings over the settings panel's
//! changes and replaces `widgets.toml`. What it replaced is kept in
//! `$XDG_DATA_HOME/mochi/bentos/<id>/before/`, and `remove` puts it back.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use mochi_core::changes::Changes;
use mochi_core::themes::{self, ThemeFile};
use mochi_core::toml::{Table, Value};
use mochi_plugins::bento::{Entry, Installed};
use mochi_plugins::install::{Curl, Installer, Mode, Outcome, Plan};
use mochi_plugins::registry::Kind;
use mochi_plugins::{Locations, Manifest, Source};

use super::fetch::{self, Fetched};
use super::manifest::{self, Bento};
use super::{client, profiles, screens, share};
use crate::settings::{Op, Store};

/// What a source holds, found by the manifest at its root.
enum Package {
    Bento(Box<Bento>),
    Theme(Box<ThemeFile>),
    Plugin(Box<Manifest>),
}

fn detect(dir: &Path) -> Result<Package, String> {
    if dir.join(manifest::FILE).is_file() {
        let bento = Bento::load(dir)?;
        mochi_core::version::supports(&bento.bento.mochi)
            .map_err(|error| format!("the bento {}: {error}", bento.bento.id))?;
        return Ok(Package::Bento(Box::new(bento)));
    }
    if dir.join(themes::MANIFEST).is_file() {
        return theme_in(dir).map(|theme| Package::Theme(Box::new(theme)));
    }
    if dir.join(mochi_plugins::manifest::FILE).is_file() {
        let manifest = Manifest::load(dir).map_err(|error| error.to_string())?;
        return Ok(Package::Plugin(Box::new(manifest)));
    }
    Err(format!(
        "found no {}, {} or {} at its root",
        manifest::FILE,
        themes::MANIFEST,
        mochi_plugins::manifest::FILE
    ))
}

/// The theme whose manifest is in `dir`, whatever the directory's name.
fn theme_in(dir: &Path) -> Result<ThemeFile, String> {
    let path = dir.join(themes::MANIFEST);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let theme = ThemeFile::parse(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    mochi_core::version::supports(&theme.theme.mochi)
        .map_err(|error| format!("the theme {}: {error}", theme.theme.id))?;
    if themes::is_bundled(&theme.theme.id) {
        return Err(format!(
            "the theme {:?} has the id of one Mochi brings; it needs another",
            theme.theme.id
        ));
    }
    Ok(theme)
}

/// What `add` and `remove` work with.
pub(super) struct Context {
    pub(super) config_file: PathBuf,
    pub(super) locations: Locations,
    yes: bool,
}

impl Context {
    fn new(config_file: &Path, yes: bool) -> Result<Self, String> {
        if !yes && !std::io::stdin().is_terminal() {
            return Err("not on a terminal, so nobody can confirm: pass --yes".into());
        }
        Ok(Self {
            config_file: config_file.to_owned(),
            locations: Locations::beside(config_file),
            yes,
        })
    }

    pub(super) fn installed(&self) -> Result<Installed, String> {
        Installed::load(&self.locations.bento).map_err(|error| error.to_string())
    }

    pub(super) fn save(&self, installed: &Installed) -> Result<(), String> {
        installed
            .save(&self.locations.bento)
            .map_err(|error| error.to_string())
    }

    fn ask(&self, question: &str) -> bool {
        if self.yes {
            return true;
        }
        eprint!("{question} [y/N] ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        matches!(answer.trim(), "y" | "Y" | "yes")
    }

    /// The settings store, refusing when `changes.toml` has changes it
    /// sets aside: laying a bento over them would lose them.
    pub(super) fn store(&self) -> Result<Store, String> {
        let (store, _) = Store::load(&self.config_file, None).map_err(|error| error.to_string())?;
        let path = Changes::path(&self.config_file);
        let saved = Changes::load(&path).map_err(|error| error.to_string())?;
        if !saved.is_empty() && !store.has_changes() {
            return Err(format!(
                "{} has changes that don't apply any more; fix or drop them in the settings panel first",
                path.display()
            ));
        }
        Ok(store)
    }
}

/// What `text` names: where from, its files, and what they hold. A
/// registry's package must be what the registry says it is.
fn get(
    context: &Context,
    text: &str,
    at: Option<&str>,
) -> Result<(Source, Fetched, Package), String> {
    let source = fetch::parse(text)?;
    let fetched = fetch::fetch(&source, &context.locations, at)?;
    let package = detect(&fetched.dir)?;
    if let Some((kind, release)) = &fetched.listed {
        let found = match &package {
            Package::Plugin(_) => Kind::Plugin,
            Package::Theme(_) => Kind::Theme,
            Package::Bento(_) => Kind::Bento,
        };
        if found != *kind {
            return Err(format!(
                "the registry lists {source} {} as a {}, but its files hold a {}",
                release.version,
                kind.as_str(),
                found.as_str()
            ));
        }
    }
    Ok((source, fetched, package))
}

/// `mochi bento add`: installs, then switches to a theme or a bento unless
/// `use_it` is off.
pub fn add(
    config_file: &Path,
    text: &str,
    at: Option<&str>,
    use_it: bool,
    yes: bool,
) -> Result<(), String> {
    let context = Context::new(config_file, yes)?;
    let (source, fetched, package) = get(&context, text, at)?;
    match package {
        Package::Theme(theme) => {
            add_theme(&context, &source, &fetched, &theme, None)?;
            let id = &theme.theme.id;
            if use_it {
                use_theme(&context, id)?;
                eprintln!(
                    "Installed the theme {} ({id}), and it's in use.",
                    theme.theme.name
                );
            } else {
                eprintln!(
                    "Installed the theme {} ({id}). `mochi bento use {id}` puts it on.",
                    theme.theme.name
                );
            }
            Ok(())
        }
        Package::Plugin(manifest) => add_plugin(&context, &source, &manifest),
        Package::Bento(bento) => add_bento(&context, &source, &fetched, &bento, use_it),
    }
}

/// Puts a theme on: `preset` in the settings' changes, in the setup in use.
fn use_theme(context: &Context, id: &str) -> Result<(), String> {
    themes::find(id)?;
    let mut store = context.store()?;
    store.change(&Op::Merge(Changes {
        config: Table::new(),
        theme: Table::from_iter([("preset".to_owned(), Value::String(id.to_owned()))]),
    }))?;
    client::reload();
    Ok(())
}

/// `mochi bento use`: switches to a bento, back to your own setup with
/// `mine`, or puts a theme on.
pub fn use_it(config_file: &Path, name: &str, on: bool) -> Result<(), String> {
    let context = Context::new(config_file, true)?;
    let installed = context.installed()?;
    match profiles::setup_of(name) {
        None => {
            profiles::switch(&context, None)?;
            eprintln!("Back to your own setup.");
        }
        Some(id) if installed.bentos.contains_key(id) => {
            if !on {
                return Err(super::OFF.to_owned());
            }
            profiles::switch(&context, Some(id))?;
            eprintln!("Using {id}. `mochi bento use mine` switches back to your own setup.");
        }
        Some(id) => {
            use_theme(&context, id)?;
            eprintln!("The theme {id} is on.");
        }
    }
    Ok(())
}

/// Copies a theme's manifest into the themes directory and records it.
fn add_theme(
    context: &Context,
    source: &Source,
    fetched: &Fetched,
    theme: &ThemeFile,
    by: Option<&str>,
) -> Result<(), String> {
    copy_theme(&fetched.dir, &theme.theme.id)?;
    record_theme(context, source, fetched, theme, by)
}

/// Records an installed theme in bento.toml.
fn record_theme(
    context: &Context,
    source: &Source,
    fetched: &Fetched,
    theme: &ThemeFile,
    by: Option<&str>,
) -> Result<(), String> {
    let mut installed = context.installed()?;
    installed.themes.insert(
        theme.theme.id.clone(),
        Entry {
            source: source.to_string(),
            revision: fetched.revision.clone(),
            version: Some(theme.theme.version.clone()),
            by: by.map(str::to_owned),
        },
    );
    context.save(&installed)
}

fn copy_theme(from: &Path, id: &str) -> Result<(), String> {
    let dir = themes::dir()
        .ok_or("no data directory: set HOME or XDG_DATA_HOME")?
        .join(id);
    let io =
        |error: std::io::Error| format!("cannot install the theme in {}: {error}", dir.display());
    std::fs::create_dir_all(&dir).map_err(io)?;
    std::fs::copy(from.join(themes::MANIFEST), dir.join(themes::MANIFEST)).map_err(io)?;
    Ok(())
}

fn remove_theme(id: &str) -> Result<(), String> {
    if let Some(dir) = themes::dir().map(|dir| dir.join(id))
        && dir.exists()
    {
        std::fs::remove_dir_all(&dir)
            .map_err(|error| format!("cannot remove {}: {error}", dir.display()))?;
    }
    Ok(())
}

/// Records a plugin and installs it, asking first. `Ok(false)` when the
/// user said no, which takes the record out again.
fn install_plugin(
    context: &Context,
    id: &str,
    source: &Source,
    revision: Option<String>,
    by: Option<&str>,
) -> Result<bool, String> {
    let mut installed = context.installed()?;
    installed.plugins.insert(
        id.to_owned(),
        Entry {
            source: source.to_string(),
            revision,
            version: None,
            by: by.map(str::to_owned),
        },
    );
    context.save(&installed)?;
    let yes = context.yes;
    let mut confirm = |plan: &Plan| {
        eprintln!();
        eprint!("{plan}");
        if yes {
            return true;
        }
        eprint!("Install {}? [y/N] ", plan.id);
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        matches!(answer.trim(), "y" | "Y" | "yes")
    };
    let mut installer = Installer {
        locations: &context.locations,
        confirm: &mut confirm,
        fetch: &Curl,
    };
    let outcome = installer.run(id, source, Mode::Install);
    let fine = matches!(
        outcome,
        Ok(Outcome::Installed { .. } | Outcome::UpToDate { .. })
    );
    let mut installed = context.installed()?;
    if fine {
        // The version the lock recorded, for `mochi bento list`.
        let lock = mochi_plugins::Lock::load(&context.locations.lock).unwrap_or_default();
        if let Some(entry) = installed.plugins.get_mut(id) {
            entry.version = lock
                .plugins
                .get(id)
                .and_then(|locked| locked.version.clone());
        }
    } else {
        installed.plugins.remove(id);
    }
    context.save(&installed)?;
    match outcome {
        Ok(Outcome::Declined) => Ok(false),
        Ok(_) => Ok(true),
        Err(error) => Err(format!("{id}: {error}")),
    }
}

fn uninstall_plugin(context: &Context, id: &str) -> Result<(), String> {
    let mut confirm = |_: &Plan| true;
    let installer = Installer {
        locations: &context.locations,
        confirm: &mut confirm,
        fetch: &Curl,
    };
    installer.remove(id).map_err(|error| error.0)?;
    Ok(())
}

/// The modules that run now, from the files and the changes.
fn modules_now(store: &Store) -> Vec<String> {
    let (config, _) = store.exported();
    config
        .get("modules")
        .and_then(Value::as_array)
        .map(|modules| {
            modules
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| mochi_core::Config::default().modules)
}

/// Changes with only `modules` set.
fn modules_change(modules: Vec<String>) -> Changes {
    Changes {
        config: Table::from_iter([(
            "modules".to_owned(),
            Value::Array(modules.into_iter().map(Value::String).collect()),
        )]),
        theme: Table::new(),
    }
}

fn add_plugin(context: &Context, source: &Source, manifest: &Manifest) -> Result<(), String> {
    let id = manifest.plugin.id.clone();
    // A release installs from its own source; a clone of it only told the id.
    let revision = None;
    if !install_plugin(context, &id, source, revision, None)? {
        eprintln!("Skipped {id}");
        return Ok(());
    }
    let mut store = context.store()?;
    let mut modules = modules_now(&store);
    if !modules.contains(&id) {
        modules.push(id.clone());
        store.change(&Op::Merge(modules_change(modules)))?;
    }
    eprintln!("Installed and turned on {} ({id})", manifest.plugin.name);
    client::reload();
    Ok(())
}
fn add_bento(
    context: &Context,
    source: &Source,
    fetched: &Fetched,
    bento: &Bento,
    use_it: bool,
) -> Result<(), String> {
    let id = bento.bento.id.clone();
    let sources = bento.sources()?;
    let brought = bento_themes(&fetched.dir)?;
    check(&fetched.dir, bento, &brought)?;
    let screens = screens::connected();

    // What it is and what it does, then one question.
    eprintln!();
    eprintln!("{} {} ({id})", bento.bento.name, bento.bento.version);
    if !bento.bento.description.is_empty() {
        eprintln!("  {}", bento.bento.description);
    }
    if !bento.bento.authors.is_empty() {
        eprintln!("  by {}", bento.bento.authors.join(", "));
    }
    match &fetched.revision {
        Some(revision) => eprintln!(
            "  from      {source} at {}",
            &revision[..revision.len().min(10)]
        ),
        None => eprintln!("  from      {source}"),
    }
    eprintln!("{}", share::summary(bento, brought.len()));
    for (plugin, plugin_source) in &sources {
        eprintln!("  needs     {plugin} from {plugin_source}, which asks before it installs");
    }
    let wanted = bento
        .widgets
        .iter()
        .filter_map(|widget| screens::parse_role(&widget.output))
        .max()
        .map_or(0, |most| most + 1);
    if wanted > screens.len() {
        eprintln!(
            "  Its widgets use {wanted} screens and {} {} connected: the widgets for the others stay hidden.",
            screens.len(),
            if screens.len() == 1 { "is" } else { "are" }
        );
    }
    let installed = context.installed()?;
    let again = installed.bentos.contains_key(&id);
    let active = installed.active.as_deref() == Some(id.as_str());
    if again {
        eprintln!("  It's installed already: this updates it.");
    }
    if active {
        eprintln!("  It's in use: its new settings go over the ones in place.");
    } else if use_it {
        eprintln!(
            "  It's a setup you switch to: its settings go over yours{}, and\n  `mochi bento use mine` switches back to your own, as you left it.",
            if bento.widgets.is_empty() {
                ""
            } else {
                ", its widgets replace yours"
            }
        );
    } else {
        eprintln!("  It's kept as a setup to switch to with `mochi bento use {id}`.");
    }
    if !context.ask(&format!("Add {id}?")) {
        eprintln!("Skipped {id}");
        return Ok(());
    }

    // Themes and plugins first, so the settings can use them; undone if
    // something fails.
    let mut done_themes = Vec::new();
    let mut done_plugins = Vec::new();
    let result = (|| -> Result<(), String> {
        for (theme_id, dir) in &brought {
            let theme = theme_in(dir)?;
            copy_theme(dir, theme_id)?;
            done_themes.push(theme_id.clone());
            record_theme(context, source, fetched, &theme, Some(&id))?;
        }
        for (plugin, plugin_source) in &sources {
            if !install_plugin(context, plugin, plugin_source, None, Some(&id))? {
                return Err(format!("{id} needs {plugin}"));
            }
            done_plugins.push(plugin.clone());
        }
        profiles::keep(&fetched.dir, &id)
    })();
    if let Err(error) = result {
        for theme_id in &done_themes {
            let _ = remove_theme(theme_id);
        }
        for plugin in &done_plugins {
            let _ = uninstall_plugin(context, plugin);
        }
        if let Ok(mut installed) = context.installed() {
            installed
                .themes
                .retain(|theme_id, _| !done_themes.contains(theme_id));
            installed
                .plugins
                .retain(|plugin, _| !done_plugins.contains(plugin));
            let _ = context.save(&installed);
        }
        if !again {
            let _ = std::fs::remove_dir_all(profiles::folder(Some(&id))?);
        }
        return Err(format!("{error}; nothing changed"));
    }

    let mut installed = context.installed()?;
    installed.bentos.insert(
        id.clone(),
        Entry {
            source: source.to_string(),
            revision: fetched.revision.clone(),
            version: Some(bento.bento.version.clone()),
            by: None,
        },
    );
    context.save(&installed)?;
    if active {
        // An update of the setup in use: its new settings over the ones in
        // place.
        profiles::lay(context, &id)?;
        client::reload();
    } else {
        // What it became last time was for the old version.
        profiles::forget(&id)?;
        if use_it {
            profiles::switch(context, Some(&id))?;
        }
    }
    if active || use_it {
        eprintln!(
            "Added {}. `mochi bento use mine` switches back to your own setup, and `mochi bento remove {id}` takes it out.",
            bento.bento.name
        );
    } else {
        eprintln!(
            "Added {}. `mochi bento use {id}` switches to it.",
            bento.bento.name
        );
    }
    Ok(())
}

/// The themes a bento brings: `themes/<id>/mochi-theme.toml`.
fn bento_themes(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let Ok(entries) = std::fs::read_dir(dir.join("themes")) else {
        return Ok(Vec::new());
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.join(themes::MANIFEST).is_file() {
            continue;
        }
        let theme = theme_in(&path)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if theme.theme.id != name {
            return Err(format!(
                "themes/{name}: its id is {:?}; the directory must be named like it",
                theme.theme.id
            ));
        }
        found.push((name, path));
    }
    found.sort();
    Ok(found)
}

/// Checks a bento's settings and theme as Mochi would read them, on their
/// own, with its plugins listed and its themes standing in for installed
/// ones.
fn check(dir: &Path, bento: &Bento, brought: &[(String, PathBuf)]) -> Result<(), String> {
    let scratch = std::env::temp_dir().join(format!("mochi-bento-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
    let result = (|| -> Result<(), String> {
        let mut theme = bento.theme.clone();
        let preset = theme
            .get("preset")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(preset) = preset
            && brought.iter().any(|(id, _)| *id == preset)
        {
            theme.insert("preset".into(), Value::String("obsidian".into()));
        }
        let mut plugins = Table::new();
        for (id, source) in &bento.plugins {
            let entry = Table::from_iter([("source".to_owned(), Value::String(source.clone()))]);
            plugins.insert(id.clone(), Value::Table(entry));
        }
        let plugins = Table::from_iter([("plugins".to_owned(), Value::Table(plugins))]);
        for (name, table) in [
            ("config.toml", &bento.config),
            ("theme.toml", &theme),
            ("plugins.toml", &plugins),
        ] {
            let text = mochi_core::toml::to_string(table).map_err(|error| error.to_string())?;
            std::fs::write(scratch.join(name), text).map_err(|error| error.to_string())?;
        }
        Store::load(&scratch.join("config.toml"), None)
            .map(drop)
            .map_err(|error| {
                // Named by the bento's sections, not the scratch files.
                let mut text = error.to_string();
                for (file, section) in [
                    ("config.toml", "[config]"),
                    ("theme.toml", "[theme]"),
                    ("plugins.toml", "[plugins]"),
                ] {
                    text = text.replace(&scratch.join(file).display().to_string(), section);
                }
                text.replace(&scratch.display().to_string(), &dir.display().to_string())
            })
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result.map_err(|error| format!("{}: {error}", manifest::FILE))
}

/// Sets the wallpaper with awww or swww when one runs; says where it is
/// otherwise.
pub(super) fn set_wallpaper(path: &Path) {
    for program in ["awww", "swww"] {
        let set = std::process::Command::new(program)
            .arg("img")
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if set.is_ok_and(|status| status.success()) {
            eprintln!("Set the wallpaper with {program}");
            return;
        }
    }
    eprintln!(
        "Its wallpaper is {}; set it with your wallpaper tool",
        path.display()
    );
}

/// `mochi bento try`: a theme, or a bento's settings and theme, applied
/// without keeping them, until Keep or Drop in the settings.
pub fn try_it(config_file: &Path, text: &str) -> Result<(), String> {
    let context = Context::new(config_file, true)?;
    let (source, fetched, package) = get(&context, text, None)?;
    let tried = match package {
        Package::Plugin(_) => {
            return Err(
                "a plugin can't be tried: `mochi bento add` installs it, and `mochi bento remove` takes it out"
                    .into(),
            );
        }
        Package::Theme(theme) => {
            add_theme(&context, &source, &fetched, &theme, None)?;
            Changes {
                config: Table::new(),
                theme: Table::from_iter([(
                    "preset".to_owned(),
                    Value::String(theme.theme.id.clone()),
                )]),
            }
        }
        Package::Bento(bento) => {
            let brought = bento_themes(&fetched.dir)?;
            check(&fetched.dir, &bento, &brought)?;
            for (_, dir) in &brought {
                let theme = theme_in(dir)?;
                copy_theme(dir, &theme.theme.id)?;
                record_theme(&context, &source, &fetched, &theme, None)?;
            }
            let catalog =
                crate::modules::catalog(config_file).map_err(|error| error.to_string())?;
            let available: Vec<String> = catalog
                .modules
                .iter()
                .map(|module| module.id().to_owned())
                .chain(catalog.listed.iter().map(|listed| listed.id.clone()))
                .collect();
            let mut config = bento.config.clone();
            let mut missing = Vec::new();
            if let Some(Value::Array(modules)) = config.get_mut("modules") {
                modules.retain(|module| {
                    let id = module.as_str().unwrap_or_default();
                    let there = available.iter().any(|available| available == id);
                    if !there {
                        missing.push(id.to_owned());
                    }
                    there
                });
            }
            if let Some(Value::Table(sections)) = config.get_mut("module") {
                sections.retain(|id, _| !missing.iter().any(|gone| gone == id));
            }
            if !missing.is_empty() {
                eprintln!(
                    "Trying it without {}, which {} to be installed: `mochi bento add` does that.",
                    missing.join(", "),
                    if missing.len() == 1 { "needs" } else { "need" }
                );
            }
            if !bento.widgets.is_empty() || bento.bento.wallpaper.is_some() {
                eprintln!("Its widgets and wallpaper only come with `mochi bento add`.");
            }
            Changes {
                config,
                theme: bento.theme.clone(),
            }
        }
    };
    let json = serde_json::json!({
        "config": serde_json::to_value(&tried.config).map_err(|error| error.to_string())?,
        "theme": serde_json::to_value(&tried.theme).map_err(|error| error.to_string())?,
    });
    client::command("settings", "try", vec![json.to_string()]).map_err(|error| {
        if error.contains("unknown module") {
            "trying needs the settings module: add \"settings\" to modules".to_owned()
        } else {
            error
        }
    })?;
    // The bar with Keep and Drop is in the settings panel.
    let _ = client::command("settings", "open", vec!["colors".into()]);
    eprintln!(
        "Trying it now: Keep or Drop it at the bottom of the settings panel, or run\n  mochi ipc settings keep    or    mochi ipc settings drop"
    );
    Ok(())
}

/// `mochi bento remove`: a bento, a theme or a plugin Bento installed.
pub fn remove(config_file: &Path, id: &str, yes: bool) -> Result<(), String> {
    let context = Context::new(config_file, yes)?;
    let mut installed = context.installed()?;
    if installed.bentos.contains_key(id) {
        return remove_bento(&context, id);
    }
    if installed.themes.contains_key(id) {
        // Off the theme first, while it can still be read.
        let mut store = context.store()?;
        let (_, theme) = store.exported();
        if theme.get("preset").and_then(Value::as_str) == Some(id) {
            store.change(&Op::Merge(Changes {
                config: Table::new(),
                theme: Table::from_iter([("preset".to_owned(), Value::String("obsidian".into()))]),
            }))?;
        }
        remove_theme(id)?;
        installed.themes.remove(id);
        context.save(&installed)?;
        eprintln!("Removed the theme {id}");
        client::reload();
        return Ok(());
    }
    if installed.plugins.contains_key(id) {
        if !context.ask(&format!("Remove the plugin {id}?")) {
            return Ok(());
        }
        // Out of the modules first, while it's still listed.
        let mut store = context.store()?;
        let modules = modules_now(&store);
        if modules.iter().any(|module| module == id) {
            let modules = modules.into_iter().filter(|module| module != id).collect();
            store.change(&Op::Merge(modules_change(modules)))?;
        }
        uninstall_plugin(&context, id)?;
        installed = context.installed()?;
        installed.plugins.remove(id);
        context.save(&installed)?;
        eprintln!("Removed the plugin {id}");
        client::reload();
        return Ok(());
    }
    Err(format!(
        "Bento didn't install anything called {id}; `mochi bento list` shows what it did"
    ))
}

fn remove_bento(context: &Context, id: &str) -> Result<(), String> {
    let installed = context.installed()?;
    // What another bento installed still needs stays.
    let mut needed = std::collections::BTreeSet::new();
    for other in installed.bentos.keys().filter(|other| *other != id) {
        let dir = profiles::stored(other)?;
        if let Ok(bento) = Bento::load(&dir) {
            needed.extend(bento.plugins.keys().cloned());
        }
        for (theme, _) in bento_themes(&dir).unwrap_or_default() {
            needed.insert(theme);
        }
    }
    let themes: Vec<String> = by(&installed.themes, id)
        .into_iter()
        .filter(|theme| !needed.contains(theme))
        .collect();
    let plugins: Vec<String> = by(&installed.plugins, id)
        .into_iter()
        .filter(|plugin| !needed.contains(plugin))
        .collect();
    let active = installed.active.as_deref() == Some(id);
    if active {
        eprintln!("{id} is in use: removing it switches back to your own setup, as you left it.");
    }
    eprintln!("Removing {id} forgets what its setup became.");
    if !themes.is_empty() {
        eprintln!("  removes the themes {}", themes.join(", "));
    }
    if !plugins.is_empty() {
        eprintln!("  removes the plugins {}", plugins.join(", "));
    }
    if !context.ask(&format!("Remove {id}?")) {
        return Ok(());
    }
    if active {
        profiles::switch(context, None)?;
    }
    for theme in &themes {
        remove_theme(theme)?;
    }
    for plugin in &plugins {
        uninstall_plugin(context, plugin)?;
    }
    let mut installed = context.installed()?;
    installed.themes.retain(|theme, _| !themes.contains(theme));
    installed
        .plugins
        .retain(|plugin, _| !plugins.contains(plugin));
    installed.bentos.remove(id);
    context.save(&installed)?;
    let _ = std::fs::remove_dir_all(profiles::folder(Some(id))?);
    eprintln!("Removed {id}");
    client::reload();
    Ok(())
}

fn by(entries: &std::collections::BTreeMap<String, Entry>, id: &str) -> Vec<String> {
    entries
        .iter()
        .filter(|(_, entry)| entry.by.as_deref() == Some(id))
        .map(|(key, _)| key.clone())
        .collect()
}

/// `mochi bento list`.
pub fn list(config_file: &Path) -> Result<(), String> {
    let locations = Locations::beside(config_file);
    let installed = Installed::load(&locations.bento).map_err(|error| error.to_string())?;
    let lock = mochi_plugins::Lock::load(&locations.lock).unwrap_or_default();
    if installed.active.is_none() && !installed.bentos.is_empty() {
        println!("Your own setup is in use.");
    }
    if installed.plugins.is_empty() && installed.themes.is_empty() && installed.bentos.is_empty() {
        println!("Bento installed nothing yet: `mochi bento add <source>` does.");
        return Ok(());
    }
    for (kind, entries) in [
        ("bentos", &installed.bentos),
        ("themes", &installed.themes),
        ("plugins", &installed.plugins),
    ] {
        if entries.is_empty() {
            continue;
        }
        println!("{kind}");
        for (id, entry) in entries {
            let version = entry
                .version
                .as_ref()
                .map(|version| format!(" {version}"))
                .unwrap_or_default();
            let with = entry
                .by
                .as_ref()
                .map(|by| format!(", with {by}"))
                .unwrap_or_default();
            let using = if kind == "bentos" && installed.active.as_deref() == Some(id.as_str()) {
                ", in use"
            } else {
                ""
            };
            println!("  {id}{version}  {}{with}{using}", entry.source);
            // What the registry withdrew, by the index last downloaded.
            let commit = match kind {
                "plugins" => lock
                    .plugins
                    .get(id)
                    .and_then(|locked| locked.commit.clone()),
                _ => entry.revision.clone(),
            };
            if let (Ok(source), Some(commit)) = (entry.source.parse::<Source>(), commit) {
                match mochi_plugins::registry::withdrawn(&locations, &source, &commit) {
                    Ok(None) => {}
                    Ok(Some(note)) | Err(note) => println!("    {note}"),
                }
            }
        }
    }
    Ok(())
}

/// `mochi bento check`: a bento, a theme or a plugin as `add` would read
/// it, without installing anything.
pub fn check_dir(dir: &Path) -> Result<(), String> {
    match detect(dir)? {
        Package::Theme(theme) => println!("the theme {} is fine", theme.theme.id),
        Package::Plugin(manifest) => println!("the plugin {} is fine", manifest.plugin.id),
        Package::Bento(bento) => {
            let brought = bento_themes(dir)?;
            check(dir, &bento, &brought)?;
            for file in bento.bento.screenshots.iter().chain(&bento.bento.wallpaper) {
                if !dir.join(file).is_file() {
                    return Err(format!("{}: {file} isn't there", manifest::FILE));
                }
            }
            println!("the bento {} is fine", bento.bento.id);
            println!("{}", share::summary(&bento, brought.len()));
        }
    }
    Ok(())
}

/// Files a theme or a bento may hold: data and pictures, nothing that
/// runs.
const DATA: [&str; 10] = [
    "toml", "md", "txt", "png", "jpg", "jpeg", "webp", "gif", "svg", "avif",
];

/// Checks that a theme's or a bento's directory holds only [`DATA`],
/// besides git's own files and a licence.
fn data_only(dir: &Path) -> Result<(), String> {
    let mut queue = vec![dir.to_owned()];
    while let Some(current) = queue.pop() {
        let entries = std::fs::read_dir(&current).map_err(|error| error.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if [".git", ".github", ".gitignore", ".gitattributes"].contains(&name.as_str()) {
                continue;
            }
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_dir() {
                queue.push(path);
                continue;
            }
            let extension = path
                .extension()
                .map(|extension| extension.to_string_lossy().to_lowercase());
            let licence = ["LICENSE", "LICENCE", "COPYING"]
                .iter()
                .any(|stem| name.starts_with(stem));
            let fits = kind.is_file()
                && (licence
                    || extension.is_some_and(|extension| DATA.contains(&extension.as_str())));
            if !fits {
                let shown = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                return Err(format!(
                    "{shown} isn't data: themes and bentos hold only {} files and a licence",
                    DATA.join(", ")
                ));
            }
        }
    }
    Ok(())
}

/// Text on cards and the accent's own text must read well, in both
/// versions of a theme.
pub(super) fn readable(theme: &ThemeFile) -> Result<(), String> {
    for (light, name) in [(false, "dark"), (true, "light")] {
        let colors = theme.colors(light);
        let get = |role: &str| colors.get(role).and_then(Value::as_str).unwrap_or_default();
        let pairs = [("foreground", "surface", 4.5), ("accent", "on_accent", 3.0)];
        for (front, back, wanted) in pairs {
            let ratio = mochi_core::palette::contrast(get(front), get(back)).unwrap_or(0.0);
            if ratio < wanted {
                return Err(format!(
                    "the theme {} ({name}): {front} on {back} has a contrast of {ratio:.1}, below {wanted}",
                    theme.theme.id
                ));
            }
        }
    }
    Ok(())
}

/// For the registry's CI: checks that the files of a release say what its
/// package file does, and that they're what `add` takes.
pub fn verify_release(
    dir: &Path,
    id: &str,
    kind: Kind,
    version: &str,
    mochi: &str,
) -> Result<String, String> {
    let same = |what: &str, found: &str, listed: &str| {
        if mochi_core::version::parse(found) == mochi_core::version::parse(listed) {
            Ok(())
        } else {
            Err(format!(
                "its manifest says {what} {found}, the registry {listed}"
            ))
        }
    };
    let found = detect(dir)?;
    match (kind, found) {
        (Kind::Plugin, Package::Plugin(manifest)) => {
            if manifest.plugin.id != id {
                return Err(format!("its manifest's id is {:?}", manifest.plugin.id));
            }
            same("version", &manifest.plugin.version, version)?;
            Ok(": build it with Nix and read the code".into())
        }
        (Kind::Theme, Package::Theme(theme)) => {
            if theme.theme.id != id {
                return Err(format!("its id is {:?}", theme.theme.id));
            }
            same("version", &theme.theme.version, version)?;
            same("mochi", &theme.theme.mochi, mochi)?;
            data_only(dir)?;
            readable(&theme)?;
            Ok(String::new())
        }
        (Kind::Bento, Package::Bento(bento)) => {
            if bento.bento.id != id {
                return Err(format!("its id is {:?}", bento.bento.id));
            }
            same("version", &bento.bento.version, version)?;
            same("mochi", &bento.bento.mochi, mochi)?;
            data_only(dir)?;
            let brought = bento_themes(dir)?;
            for (_, theme_dir) in &brought {
                readable(&theme_in(theme_dir)?)?;
            }
            check_dir(dir).map(|()| String::new())
        }
        (kind, _) => Err(format!(
            "the registry lists a {}, but its files hold something else",
            kind.as_str()
        )),
    }
}

/// `mochi bento update`: plugins to their newest release, and themes and
/// bentos whose source moved on. Without ids, everything Bento installed
/// from a repository or the registry; a directory only when named. A
/// bento asks before laying its settings over yours again, and a theme or
/// a plugin a bento brought follows its bento.
pub fn update(config_file: &Path, ids: &[String], yes: bool) -> Result<(), String> {
    let context = Context::new(config_file, yes)?;
    let installed = context.installed()?;
    let wanted = |id: &str, entry: &Entry| {
        if ids.is_empty() {
            entry.by.is_none() && !entry.source.starts_with("path:")
        } else {
            ids.iter().any(|wanted| wanted == id)
        }
    };
    let mut failed = Vec::new();
    let mut changed = false;

    let yes = context.yes;
    let mut confirm = |plan: &Plan| {
        eprintln!();
        eprint!("{plan}");
        if yes {
            return true;
        }
        eprint!("Update {}? [y/N] ", plan.id);
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        matches!(answer.trim(), "y" | "Y" | "yes")
    };
    let mut installer = Installer {
        locations: &context.locations,
        confirm: &mut confirm,
        fetch: &Curl,
    };
    for (id, entry) in &installed.plugins {
        if !wanted(id, entry) {
            continue;
        }
        let source: Source = entry
            .source
            .parse()
            .map_err(|error| format!("bento.toml: {error}"))?;
        match installer.run(id, &source, Mode::Update) {
            Ok(Outcome::Installed { revision }) => {
                changed = true;
                eprintln!("updated {id} to {}", revision.unwrap_or_default());
            }
            Ok(Outcome::UpToDate { .. }) => eprintln!("{id} is up to date"),
            Ok(Outcome::Declined) => eprintln!("skipped {id}"),
            Err(error) => {
                eprintln!("mochi: {id}: {error}");
                failed.push(id.clone());
            }
        }
    }

    let others = installed
        .themes
        .iter()
        .map(|(id, entry)| (id, entry, Kind::Theme))
        .chain(
            installed
                .bentos
                .iter()
                .map(|(id, entry)| (id, entry, Kind::Bento)),
        );
    for (id, entry, kind) in others {
        if !wanted(id, entry) {
            continue;
        }
        let result = (|| -> Result<bool, String> {
            let source: Source = entry
                .source
                .parse()
                .map_err(|error| format!("bento.toml: {error}"))?;
            let fetched = fetch::fetch(&source, &context.locations, None)?;
            if fetched.revision.is_some() && fetched.revision == entry.revision {
                eprintln!("{id} is up to date");
                return Ok(false);
            }
            match (kind, detect(&fetched.dir)?) {
                (Kind::Theme, Package::Theme(theme)) => {
                    add_theme(&context, &source, &fetched, &theme, None)?;
                    eprintln!("updated the theme {id} to {}", theme.theme.version);
                    Ok(true)
                }
                (Kind::Bento, Package::Bento(bento)) => {
                    add_bento(&context, &source, &fetched, &bento, false)?;
                    Ok(true)
                }
                _ => Err(format!("{} holds something else now", entry.source)),
            }
        })();
        match result {
            Ok(true) => changed = true,
            Ok(false) => {}
            Err(error) => {
                eprintln!("mochi: {id}: {error}");
                failed.push(id.clone());
            }
        }
    }
    if changed {
        client::reload();
    }
    match failed.len() {
        0 => Ok(()),
        1 => Err(format!("{} didn't update", failed[0])),
        _ => Err(format!("{} didn't update", failed.join(", "))),
    }
}

/// `mochi bento plan`: what `add` would do, as JSON, changing nothing. The
/// settings panel shows it before asking, as the terminal shows the same
/// before its question; `add --at` then installs the commit it showed.
pub fn plan(config_file: &Path, text: &str) -> Result<(), String> {
    let context = Context::new(config_file, true)?;
    let (source, fetched, package) = get(&context, text, None)?;
    let mut out = serde_json::json!({
        "source": source.to_string(),
        "at": fetched.revision,
    });
    let about =
        |kind: &str, id: &str, name: &str, version: &str, description: &str, authors: &[String]| {
            serde_json::json!({
                "kind": kind,
                "id": id,
                "name": name,
                "version": version,
                "description": description,
                "authors": authors,
            })
        };
    let details = match package {
        Package::Plugin(manifest) => {
            let info = &manifest.plugin;
            let mut details = about(
                "plugin",
                &info.id,
                &info.name,
                &info.version,
                &info.description,
                &info.authors,
            );
            details["plugin"] = plugin_json(&manifest, &fetched.dir, &source);
            details
        }
        Package::Theme(theme) => {
            let info = &theme.theme;
            let mut details = about(
                "theme",
                &info.id,
                &info.name,
                &info.version,
                &info.description,
                &info.authors,
            );
            details["theme"] = serde_json::json!({
                "dark": theme.colors(false),
                "light": theme.colors(true),
                "sets": theme.look().keys().cloned().collect::<Vec<_>>(),
            });
            details
        }
        Package::Bento(bento) => {
            let info = &bento.bento;
            let mut details = about(
                "bento",
                &info.id,
                &info.name,
                &info.version,
                &info.description,
                &info.authors,
            );
            let brought = bento_themes(&fetched.dir)?;
            let problem = check(&fetched.dir, &bento, &brought).err();
            let mut plugins = Vec::new();
            for (id, plugin_source) in bento.sources()? {
                let described =
                    fetch::fetch(&plugin_source, &context.locations, None).and_then(|plugin| {
                        match detect(&plugin.dir)? {
                            Package::Plugin(manifest) => {
                                Ok(plugin_json(&manifest, &plugin.dir, &plugin_source))
                            }
                            _ => Err(format!("{plugin_source} isn't a plugin")),
                        }
                    });
                plugins.push(match described {
                    Ok(plan) => serde_json::json!({ "id": id, "source": plugin_source.to_string(), "plan": plan }),
                    Err(error) => serde_json::json!({ "id": id, "source": plugin_source.to_string(), "problem": error }),
                });
            }
            let mut settings = Vec::new();
            share::leaves(&bento.config, &mut Vec::new(), &mut settings);
            let mut look = Vec::new();
            share::leaves(&bento.theme, &mut Vec::new(), &mut look);
            let screens = bento
                .widgets
                .iter()
                .filter_map(|widget| screens::parse_role(&widget.output))
                .max()
                .map_or(0, |most| most + 1);
            details["bento"] = serde_json::json!({
                "settings": settings.len(),
                "theme_settings": look.len(),
                "themes": brought.iter().map(|(id, _)| id).collect::<Vec<_>>(),
                "plugins": plugins,
                "widgets": bento.widgets.len(),
                "screens": screens,
                "screens_here": screens::connected().len(),
                "wallpaper": bento.bento.wallpaper.is_some(),
                "installed": context.installed()?.bentos.contains_key(&info.id),
                "problem": problem,
            });
            details
        }
    };
    if let (Some(out), Some(details)) = (out.as_object_mut(), details.as_object()) {
        out.extend(details.clone());
    }
    println!("{out}");
    Ok(())
}

/// What a plugin will run and read, as the terminal's question shows it.
fn plugin_json(manifest: &Manifest, dir: &Path, source: &Source) -> serde_json::Value {
    let backend = manifest.backend.as_ref();
    // An archive with its backend built installs as it is, like a release.
    let built = match source {
        Source::GitRelease { .. } => true,
        Source::Archive { .. } => backend.is_some_and(|backend| dir.join(&backend.exec).is_file()),
        _ => false,
    };
    let runs = if built {
        None
    } else {
        mochi_plugins::install::how_to_build(manifest, dir).map(|build| build.to_string())
    };
    serde_json::json!({
        "runs": runs,
        "downloads": built,
        "starts": backend.map(|backend| backend.exec.clone()),
        "needs": backend.map(|backend| backend.needs.clone()).unwrap_or_default(),
        "missing": manifest.missing_needs(),
        "reads": manifest.uses.state,
        "replaces": manifest.views.overrides,
        "actions": manifest.actions.iter().map(|action| action.name.clone()).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const THEME: &str = r##"
[theme]
id = "dusk"
name = "Dusk"
version = "0.2.0"
mochi = "0.0.1"

[dark]
accent = "#c792ea"
surface = "#1b1726"
"##;

    fn dir(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mochi-verify-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    #[test]
    fn a_release_must_match_the_registry_and_hold_only_data() {
        let fine = dir("fine", &[(themes::MANIFEST, THEME), ("README.md", "Dusk")]);
        let verify =
            |dir: &Path, version: &str| verify_release(dir, "dusk", Kind::Theme, version, "0.0.1");
        assert_eq!(verify(&fine, "0.2.0"), Ok(String::new()));
        assert!(
            verify(&fine, "0.3.0")
                .unwrap_err()
                .contains("version 0.2.0")
        );
        assert!(
            verify_release(&fine, "dusk", Kind::Plugin, "0.2.0", "0.0.1")
                .unwrap_err()
                .contains("something else")
        );

        let script = dir(
            "script",
            &[(themes::MANIFEST, THEME), ("install.sh", "rm -rf ~")],
        );
        assert!(
            verify(&script, "0.2.0")
                .unwrap_err()
                .contains("install.sh isn't data")
        );

        let dim = dir(
            "dim",
            &[(themes::MANIFEST, &THEME.replace("#1b1726", "#ffffff"))],
        );
        assert!(verify(&dim, "0.2.0").unwrap_err().contains("contrast"));

        for dir in [fine, script, dim] {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

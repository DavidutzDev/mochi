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
use mochi_module_widgets::layout::Layout;
use mochi_plugins::bento::{Entry, Installed};
use mochi_plugins::install::{Installer, Mode, Outcome, Plan};
use mochi_plugins::{Locations, Manifest, Source};

use super::fetch::{self, Fetched};
use super::manifest::{self, Bento};
use super::{client, screens, share};
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
struct Context {
    config_file: PathBuf,
    locations: Locations,
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

    fn installed(&self) -> Result<Installed, String> {
        Installed::load(&self.locations.bento).map_err(|error| error.to_string())
    }

    fn save(&self, installed: &Installed) -> Result<(), String> {
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
    fn store(&self) -> Result<Store, String> {
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

/// `mochi bento add`.
pub fn add(config_file: &Path, text: &str, yes: bool) -> Result<(), String> {
    let context = Context::new(config_file, yes)?;
    let source = fetch::parse(text)?;
    let fetched = fetch::fetch(&source)?;
    match detect(&fetched.dir)? {
        Package::Theme(theme) => {
            add_theme(&context, &source, &fetched, &theme, None)?;
            let id = &theme.theme.id;
            eprintln!(
                "Installed the theme {} ({id}). Pick it in the settings, or run\n  mochi ipc settings set theme.preset '\"{id}\"'",
                theme.theme.name
            );
            Ok(())
        }
        Package::Plugin(manifest) => add_plugin(&context, &source, &manifest),
        Package::Bento(bento) => add_bento(&context, &source, &fetched, &bento),
    }
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

/// Where Bento keeps a bento's things: what it replaced, and its
/// wallpaper.
fn bento_dir(id: &str) -> Result<PathBuf, String> {
    mochi_core::config::data_dir()
        .map(|data| data.join("bentos").join(id))
        .ok_or_else(|| "no data directory: set HOME or XDG_DATA_HOME".into())
}

fn add_bento(
    context: &Context,
    source: &Source,
    fetched: &Fetched,
    bento: &Bento,
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
    let mut installed = context.installed()?;
    let again = installed.bentos.contains_key(&id);
    if again {
        eprintln!("  It's installed already: this updates it.");
    }
    eprintln!(
        "  Its settings go over yours, in changes.toml{}. `mochi bento remove {id}`\n  puts back what it replaces.",
        if bento.widgets.is_empty() {
            ""
        } else {
            ", and its widgets replace yours"
        }
    );
    if !context.ask(&format!("Add {id}?")) {
        eprintln!("Skipped {id}");
        return Ok(());
    }

    // What it replaces, the first time only, so an update still goes back
    // to before.
    let home = bento_dir(&id)?;
    let before = home.join("before");
    let changes_file = Changes::path(&context.config_file);
    let widgets_file = context
        .config_file
        .with_file_name(mochi_module_widgets::FILE);
    if !again {
        let _ = std::fs::remove_dir_all(&before);
        std::fs::create_dir_all(&before).map_err(|error| error.to_string())?;
        for file in [&changes_file, &widgets_file] {
            if file.exists() {
                let name = file.file_name().expect("a file name");
                std::fs::copy(file, before.join(name)).map_err(|error| error.to_string())?;
            }
        }
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
        let mut store = context.store()?;
        store.change(&Op::Merge(Changes {
            config: bento.config.clone(),
            theme: bento.theme.clone(),
        }))?;
        Ok(())
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
            let _ = std::fs::remove_dir_all(&home);
        }
        return Err(format!("{error}; nothing changed"));
    }

    if !bento.widgets.is_empty() {
        let mut layout = Layout::default();
        for widget in &bento.widgets {
            let mut widget = widget.clone();
            if let Some(name) =
                screens::parse_role(&widget.output).and_then(|index| screens.get(index))
            {
                widget.output = name.clone();
            }
            layout.widgets.push(widget);
        }
        layout
            .save(&widgets_file)
            .map_err(|error| format!("cannot write {}: {error}", widgets_file.display()))?;
    }

    if let Some(file) = &bento.bento.wallpaper {
        let from = fetched.dir.join(file);
        let name = Path::new(file)
            .file_name()
            .expect("checked inside the bento");
        let to = home.join(name);
        match std::fs::copy(&from, &to) {
            Ok(_) => set_wallpaper(&to),
            Err(error) => eprintln!("No wallpaper: cannot copy {}: {error}", from.display()),
        }
    }

    installed = context.installed()?;
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
    eprintln!(
        "Added {}. `mochi bento remove {id}` takes it out.",
        bento.bento.name
    );
    client::reload();
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
fn set_wallpaper(path: &Path) {
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
    let source = fetch::parse(text)?;
    let fetched = fetch::fetch(&source)?;
    let tried = match detect(&fetched.dir)? {
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
    let themes: Vec<String> = by(&installed.themes, id);
    let plugins: Vec<String> = by(&installed.plugins, id);
    eprintln!(
        "Removing the bento {id} puts back your settings and widgets as they were before it,"
    );
    eprintln!("dropping changes made since.");
    if !themes.is_empty() {
        eprintln!("  removes the themes {}", themes.join(", "));
    }
    if !plugins.is_empty() {
        eprintln!("  removes the plugins {}", plugins.join(", "));
    }
    if !context.ask(&format!("Remove {id}?")) {
        return Ok(());
    }
    let home = bento_dir(id)?;
    let before = home.join("before");
    for file in [
        Changes::path(&context.config_file),
        context
            .config_file
            .with_file_name(mochi_module_widgets::FILE),
    ] {
        let kept = before.join(file.file_name().expect("a file name"));
        let restored = if kept.exists() {
            std::fs::copy(&kept, &file).map(drop)
        } else {
            std::fs::remove_file(&file).or_else(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(error)
                }
            })
        };
        restored.map_err(|error| format!("cannot put back {}: {error}", file.display()))?;
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
    let _ = std::fs::remove_dir_all(&home);
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
    let installed = Installed::load(&Locations::beside(config_file).bento)
        .map_err(|error| error.to_string())?;
    if installed == Installed::default() {
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
            println!("  {id}{version}  {}{with}", entry.source);
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

//! The built-in modules and the installed plugins, reading `config.toml`
//! against them, and starting them. Modules start at launch and again on
//! `mochi reload`, so this lives apart from the daemon loop that calls it.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::Context;
use mochi_core::assets::ShellModule;
use mochi_core::assets::{self, Mode};
use mochi_core::compositor::Compositor;
use mochi_core::{
    ActivityIds, Config, ConfigError, Module, ModuleCtx, ModuleRequest, Paths, actions, examples,
};
use mochi_plugins::Locations;
use mochi_protocol::Contribution;
use tokio::sync::mpsc::UnboundedSender;

use crate::daemon::{ModuleExit, ModuleSlot};
use crate::plugins::PluginModule;

/// What a generated `config.toml` turns on: the whole shell.
pub const DEFAULT_MODULES: [&str; 19] = [
    "idle",
    "osd",
    "workspaces",
    "media",
    "audio",
    "notifications",
    "launcher",
    "hub",
    "power",
    "capture",
    "share",
    "clipboard",
    "tray",
    "network",
    "bluetooth",
    "battery",
    "performance",
    "widgets",
    "notes",
];

/// Every module compiled into this binary, fresh: a module runs once, so a
/// restart takes a new instance.
pub fn builtin() -> Vec<Box<dyn Module>> {
    #[allow(unused_mut)]
    let mut modules: Vec<Box<dyn Module>> = vec![
        Box::new(mochi_module_idle::Idle),
        Box::new(mochi_module_osd::Osd),
        Box::new(mochi_module_workspaces::Workspaces),
        Box::new(mochi_module_media::Media),
        Box::new(mochi_module_audio::Audio),
        Box::new(mochi_module_notifications::Notifications),
        Box::new(mochi_module_launcher::Launcher),
        Box::new(mochi_module_hub::Hub),
        Box::new(mochi_module_power::Power),
        Box::new(mochi_module_capture::Capture),
        Box::new(mochi_module_share::Share),
        Box::new(mochi_module_clipboard::Clipboard),
        Box::new(mochi_module_tray::Tray),
        Box::new(mochi_module_network::Network),
        Box::new(mochi_module_bluetooth::Bluetooth),
        Box::new(mochi_module_battery::BatteryModule),
        Box::new(mochi_module_performance::Performance),
        Box::new(mochi_module_widgets::Widgets),
        Box::new(mochi_module_notes::Notes),
    ];
    #[cfg(feature = "demo")]
    modules.push(Box::new(mochi_module_demo::Demo));
    modules
}

/// A plugin from plugins.toml, and why it can't run when it can't.
#[derive(Debug, Clone)]
pub struct Listed {
    pub id: String,
    pub problem: Option<String>,
}

/// Every module there is: the builtins, and the plugins plugins.toml lists
/// that are installed and well-formed.
#[allow(missing_debug_implementations)]
pub struct Catalog {
    pub modules: Vec<Box<dyn Module>>,
    pub plugins: BTreeMap<&'static str, PluginModule>,
    pub listed: Vec<Listed>,
}

/// Reads plugins.toml next to `config_file` and the manifests it leads to.
/// A plugins.toml with an error is a config error; a plugin that isn't
/// installed, or has a bad manifest, is only listed with its problem.
pub fn catalog(config_file: &Path) -> Result<Catalog, ConfigError> {
    let locations = Locations::beside(config_file);
    let found = mochi_plugins::discover(&locations)
        .map_err(|error| ConfigError::invalid(&locations.list, error.to_string()))?;
    let mut modules = builtin();
    let mut plugins = BTreeMap::new();
    let mut listed = Vec::new();
    for found in found {
        let checked = found.manifest.and_then(|manifest| {
            for spec in manifest.actions() {
                actions::validate(&spec)
                    .map_err(|error| format!("its manifest declares an invalid action: {error}"))?;
            }
            Ok(manifest)
        });
        let problem = match checked {
            Ok(manifest) => {
                let plugin = PluginModule::new(found.dir, manifest);
                plugins.insert(plugin.id(), plugin.clone());
                modules.push(Box::new(plugin));
                None
            }
            Err(problem) => Some(problem),
        };
        listed.push(Listed {
            id: found.id,
            problem,
        });
    }
    Ok(Catalog {
        modules,
        plugins,
        listed,
    })
}

/// Reads `config.toml` and checks everything in it: module names, every
/// module's settings, and the actions the enabled modules declare.
/// `modules` replaces the file's module list, like `--modules` does. A
/// plugin plugins.toml lists counts as a module even before it's
/// installed, so the file stays valid while it isn't.
pub fn load_config(path: &Path, modules: Option<&[String]>) -> Result<Config, ConfigError> {
    let mut config = Config::load(path)?;
    if let Some(modules) = modules {
        config.modules = modules.to_vec();
    }
    let catalog = catalog(path)?;
    let builtin = catalog.modules;
    let available: Vec<&str> = builtin
        .iter()
        .map(|module| module.id())
        .chain(catalog.listed.iter().map(|listed| listed.id.as_str()))
        .collect();
    config.check(&available, path)?;
    for module in &builtin {
        module
            .check_settings(&config.settings(module.id()))
            .map_err(|error| {
                ConfigError::invalid(path, format!("[module.{}]: {error}", module.id()))
            })?;
        if config.modules.iter().any(|id| id == module.id()) {
            for spec in module.actions() {
                actions::validate(&spec).map_err(|error| {
                    ConfigError::invalid(
                        path,
                        format!("module {} declares an invalid action: {error}", module.id()),
                    )
                })?;
            }
        }
    }
    Ok(config)
}

/// The `config.toml` a new user gets.
pub fn example_config() -> String {
    let builtin = builtin();
    let sections: Vec<(&str, &str)> = builtin
        .iter()
        .map(|module| (module.id(), module.settings_example()))
        .collect();
    let enabled: Vec<&str> = DEFAULT_MODULES
        .into_iter()
        .filter(|id| sections.iter().any(|(available, _)| available == id))
        .collect();
    examples::config(&enabled, &sections)
}

/// Writes the example `config.toml` and `theme.toml` next to each other,
/// each only if it's missing. Returns the files written.
pub fn write_examples(config_file: &Path) -> io::Result<Vec<PathBuf>> {
    let theme_file = config_file.with_file_name("theme.toml");
    let mut written = Vec::new();
    for (path, text) in [
        (config_file.to_owned(), example_config()),
        (theme_file, examples::THEME.to_owned()),
    ] {
        if path.exists() {
            continue;
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, text)?;
        written.push(path);
    }
    Ok(written)
}

/// Starts modules and writes their views.
#[derive(Debug)]
pub struct Runner {
    pub paths: Paths,
    /// Where `config.toml` is, for modules that keep files next to it.
    pub config_dir: PathBuf,
    pub mode: Mode,
    pub compositor: Compositor,
    pub ids: ActivityIds,
    pub requests: UnboundedSender<ModuleRequest>,
    pub exits: UnboundedSender<ModuleExit>,
    /// Counts starts, so the exit of a module replaced on reload isn't
    /// mistaken for its successor's.
    generation: u64,
}

impl Runner {
    pub fn new(
        paths: Paths,
        mode: Mode,
        compositor: Compositor,
        requests: UnboundedSender<ModuleRequest>,
        exits: UnboundedSender<ModuleExit>,
    ) -> Self {
        Self {
            config_dir: paths.config_dir.clone(),
            paths,
            mode,
            compositor,
            ids: ActivityIds::default(),
            requests,
            exits,
            generation: 0,
        }
    }

    /// Writes the core QML and these modules' views, with the files that
    /// override some of them.
    pub fn write_shell(
        &self,
        modules: &[&dyn Module],
        overrides: &BTreeMap<&'static str, Vec<(String, PathBuf)>>,
    ) -> anyhow::Result<()> {
        let assets: Vec<mochi_core::Assets> =
            modules.iter().map(|module| module.assets()).collect();
        let views: Vec<ShellModule<'_>> = modules
            .iter()
            .zip(&assets)
            .map(|(module, assets)| ShellModule {
                id: module.id(),
                assets,
                overrides: overrides
                    .get(module.id())
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
            })
            .collect();
        let dir = self.paths.shell_dir();
        let written = assets::write_shell(&dir, &mochi_core::QML, &views, self.mode)
            .with_context(|| format!("cannot write {}", dir.display()))?;
        tracing::info!(mode = ?self.mode, written, dir = %dir.display(), "wrote the shell");
        Ok(())
    }

    /// Starts one module with its settings, on a task of its own.
    pub fn start(
        &mut self,
        module: Box<dyn Module>,
        settings: mochi_core::toml::Table,
    ) -> anyhow::Result<ModuleSlot> {
        let id = module.id();
        // Whatever an earlier run left there is stale.
        let data_dir = self.paths.data_dir(id);
        if data_dir.exists() {
            std::fs::remove_dir_all(&data_dir)
                .with_context(|| format!("cannot empty {}", data_dir.display()))?;
        }
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("cannot create {}", data_dir.display()))?;

        let (ctx, events) = ModuleCtx::new(
            id,
            settings,
            self.compositor.clone(),
            self.ids.clone(),
            data_dir,
            self.paths.session_dir(id),
            self.requests.clone(),
        );
        let ctx = ctx.with_config_dir(self.config_dir.clone());
        self.generation += 1;
        let slot = ModuleSlot {
            contributions: contributions(module.as_ref()),
            assets: module.assets(),
            actions: module.actions(),
            events: Some(events),
            generation: self.generation,
        };

        let generation = self.generation;
        let task = tokio::spawn(module.run(ctx));
        let exits = self.exits.clone();
        tokio::spawn(async move {
            let _ = exits.send((id, generation, task.await));
        });
        Ok(slot)
    }
}

/// What a module offers others, without the ones whose view it doesn't
/// ship: those are bugs in the module, logged here. Some kinds, like a
/// launcher provider, have no view.
fn contributions(module: &dyn Module) -> Vec<Contribution> {
    let assets = module.assets();
    module
        .contributions()
        .into_iter()
        .filter(|spec| {
            let found = spec.view.is_empty() || assets.has_view(&spec.view);
            if !found {
                tracing::error!(module = module.id(), view = %spec.view, "the module offers a view it doesn't have");
            }
            found
        })
        .map(|spec| spec.into_contribution(module.id()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `mochi plugins` refuses these ids without mochid at hand.
    #[test]
    fn the_plugins_crate_knows_every_builtin() {
        let mut ids: Vec<&str> = builtin().iter().map(|module| module.id()).collect();
        if !ids.contains(&"demo") {
            ids.push("demo");
        }
        ids.sort_unstable();
        let mut known = mochi_plugins::BUILTIN.to_vec();
        known.sort_unstable();
        assert_eq!(ids, known);
    }
}

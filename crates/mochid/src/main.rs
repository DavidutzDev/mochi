//! The Mochi daemon. It owns all state, runs the modules, decides what the
//! island shows, and keeps the Quickshell UI running.

mod daemon;
mod ipc;

use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::Parser;
use mochi_core::assets::{self, Mode};
use mochi_core::supervisor::{self, Supervisor, UiCommand};
use mochi_core::{ActivityIds, Bubbles, Config, Module, ModuleCtx, Paths, actions};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use crate::daemon::{Daemon, Inputs, ModuleSlot};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Link the QML to the source tree instead of copying it, so edits
    /// hot-reload.
    #[arg(long)]
    dev: bool,

    /// Read this config.toml instead of ~/.config/mochi/config.toml. The
    /// theme.toml next to it is used too.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable these modules instead of the list in config.toml.
    #[arg(long, value_name = "IDS", value_delimiter = ',')]
    modules: Option<Vec<String>>,

    /// Keep the socket and the generated shell here instead of
    /// $XDG_RUNTIME_DIR/mochi, for example to run a second daemon in tests.
    #[arg(long, value_name = "DIR")]
    runtime_dir: Option<PathBuf>,

    /// The Quickshell executable.
    #[arg(long, value_name = "PATH", default_value = "quickshell")]
    quickshell: OsString,
}

fn main() -> ExitCode {
    init_logging();

    let args = Args::parse();
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "could not start the async runtime");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Logs to stderr, filtered by `MOCHI_LOG` (default `info`). Colors only on a
/// terminal, and no timestamps under systemd, whose journal adds its own.
fn init_logging() {
    let filter = EnvFilter::try_from_env("MOCHI_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let logs = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal());
    if std::env::var_os("JOURNAL_STREAM").is_some() {
        logs.without_time().init();
    } else {
        logs.init();
    }
}

/// Every module compiled into this binary.
fn builtin_modules() -> Vec<Box<dyn Module>> {
    #[allow(unused_mut)]
    let mut modules: Vec<Box<dyn Module>> = vec![
        Box::new(mochi_module_idle::Idle),
        Box::new(mochi_module_osd::Osd),
        Box::new(mochi_module_workspaces::Workspaces),
        Box::new(mochi_module_media::Media),
    ];
    #[cfg(feature = "demo")]
    modules.push(Box::new(mochi_module_demo::Demo));
    modules
}

async fn run(args: Args) -> anyhow::Result<()> {
    let mut paths = Paths::from_env()?;
    if let Some(dir) = args.runtime_dir {
        paths.runtime_dir = dir;
    }
    let config_file = args.config.unwrap_or_else(|| paths.config_file());
    let theme_file = config_file.with_file_name("theme.toml");

    let mut config = Config::load(&config_file)?;
    if let Some(modules) = args.modules {
        config.modules = modules;
    }
    let mut builtin = builtin_modules();
    let available: Vec<&str> = builtin.iter().map(|module| module.id()).collect();
    config.check(&available, &config_file)?;
    let theme = mochi_core::config::load_theme(&theme_file)?;

    // Claim the socket before touching anything a running daemon uses.
    std::fs::create_dir_all(&paths.runtime_dir)
        .with_context(|| format!("cannot create {}", paths.runtime_dir.display()))?;
    let listener = ipc::bind(&paths.socket())
        .await
        .with_context(|| format!("cannot listen on {}", paths.socket().display()))?;

    supervisor::check_version(&args.quickshell)?;

    // Keep the enabled modules, in config order.
    let mut enabled = Vec::new();
    for id in &config.modules {
        let index = builtin
            .iter()
            .position(|module| module.id() == id)
            .expect("checked against the available modules");
        enabled.push(builtin.swap_remove(index));
    }
    for module in &enabled {
        for spec in module.actions() {
            if let Err(error) = actions::validate(&spec) {
                bail!("module {} declares an invalid action: {error}", module.id());
            }
        }
    }

    let mode = if args.dev { Mode::Link } else { Mode::Copy };
    let views: Vec<(&str, mochi_core::Assets)> = enabled
        .iter()
        .map(|module| (module.id(), module.assets()))
        .collect();
    let written = assets::write_shell(&paths.shell_dir(), mochi_core::QML, &views, mode)
        .with_context(|| format!("cannot write {}", paths.shell_dir().display()))?;
    tracing::info!(?mode, written, dir = %paths.shell_dir().display(), "wrote the shell");

    let (connection_sender, connections) = mpsc::unbounded_channel();
    let (request_sender, requests) = mpsc::unbounded_channel();
    let (exit_sender, exits) = mpsc::unbounded_channel();
    let (ui_sender, ui) = mpsc::unbounded_channel();

    // Without a supported compositor this logs why and returns a handle whose
    // state says so; modules that need it stay idle.
    let compositor = mochi_core::compositor::connect();

    let ids = ActivityIds::default();
    let mut slots = Vec::new();
    for module in enabled {
        let id = module.id();
        let (ctx, events) = ModuleCtx::new(
            id,
            config.settings(id),
            compositor.clone(),
            ids.clone(),
            request_sender.clone(),
        );
        slots.push((
            id,
            ModuleSlot {
                assets: module.assets(),
                actions: module.actions(),
                events: Some(events),
            },
        ));

        let task = tokio::spawn(module.run(ctx));
        let exits = exit_sender.clone();
        tokio::spawn(async move {
            let _ = exits.send((id, task.await));
        });
    }
    tracing::info!(modules = ?config.modules, "started modules");

    ipc::accept_all(listener, connection_sender);
    let supervisor = Supervisor::spawn(
        UiCommand {
            program: args.quickshell,
            shell_dir: paths.shell_dir(),
            socket: paths.socket(),
        },
        ui_sender,
    )?;

    let bubbles = Bubbles::new(
        config.bubbles.modules.clone(),
        Some(config.bubbles.max_per_area),
    );
    let daemon = Daemon::new(slots, bubbles, theme, theme_file, supervisor, compositor);
    let result = daemon
        .run(Inputs {
            connections,
            requests,
            exits,
            ui,
        })
        .await;

    let _ = std::fs::remove_file(paths.socket());
    result
}

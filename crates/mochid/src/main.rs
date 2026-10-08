//! The Mochi daemon. It owns all state, runs the modules, decides what the
//! island shows, and keeps the Quickshell UI running.

mod daemon;
mod ipc;
mod modules;
mod plugins;
mod settings;
#[cfg(test)]
mod views_check;

use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};
use mochi_core::assets::Mode;
use mochi_core::supervisor::{self, Supervisor, UiCommand};
use mochi_core::{Paths, examples};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use crate::daemon::{Daemon, Files, Inputs};
use crate::modules::Runner;
use crate::settings::Store;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Link the QML to the source tree instead of copying it, so edits
    /// hot-reload.
    #[arg(long)]
    dev: bool,

    /// Read this config.toml instead of ~/.config/mochi/config.toml. The
    /// theme.toml next to it is used too.
    #[arg(long, value_name = "FILE", global = true)]
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

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Work with config.toml and theme.toml without running the daemon.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Write the commented example files where they're missing. Never
    /// overwrites.
    Init {
        /// Print them instead of writing.
        #[arg(long)]
        print: bool,
    },
    /// Check both files, with the errors mochid would give.
    Check,
    /// Print where the files are.
    Path,
}

fn main() -> ExitCode {
    init_logging();

    let args = Args::parse();
    if let Some(Command::Config { action }) = &args.command {
        return config_main(action, args.config.clone());
    }
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

/// The default config.toml, or the one `--config` names.
fn config_file(config: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    match config {
        Some(file) => Ok(file),
        None => Ok(Paths::from_env()?.config_file()),
    }
}

/// `mochid config`, which answers on the terminal like any command-line
/// tool.
#[allow(clippy::print_stderr)]
fn config_main(action: &ConfigAction, config: Option<PathBuf>) -> ExitCode {
    match config_command(action, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mochid: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::print_stdout)]
fn config_command(action: &ConfigAction, config: Option<PathBuf>) -> anyhow::Result<()> {
    let config_file = config_file(config)?;
    let theme_file = config_file.with_file_name("theme.toml");
    match action {
        ConfigAction::Init { print: true } => {
            println!("# {}\n", config_file.display());
            print!("{}", modules::example_config());
            println!("\n# {}\n", theme_file.display());
            print!("{}", examples::THEME);
        }
        ConfigAction::Init { print: false } => {
            let written = modules::write_examples(&config_file)
                .with_context(|| format!("cannot write next to {}", config_file.display()))?;
            for file in [&config_file, &theme_file] {
                let verb = if written.contains(file) {
                    "wrote"
                } else {
                    "kept"
                };
                println!("{verb} {}", file.display());
            }
        }
        ConfigAction::Check => {
            modules::load_config(&config_file, None)?;
            mochi_core::config::load_theme(&theme_file)?;
            let widgets_file = config_file.with_file_name(mochi_module_widgets::FILE);
            let widgets = mochi_module_widgets::check(&widgets_file).map_err(anyhow::Error::msg)?;
            println!("{} is fine", config_file.display());
            println!("{} is fine", theme_file.display());
            for note in mochi_core::config::theme_notes(&theme_file)? {
                println!("  {note}; the old name still works for now");
            }
            if widgets {
                println!("{} is fine", widgets_file.display());
            }
        }
        ConfigAction::Path => {
            println!("{}", config_file.display());
            println!("{}", theme_file.display());
        }
    }
    Ok(())
}

async fn run(args: Args) -> anyhow::Result<()> {
    let mut paths = Paths::from_env()?;
    if let Some(dir) = args.runtime_dir {
        paths.runtime_dir = dir;
    }
    // A new user gets commented examples to start from. A file named with
    // --config is the caller's business.
    if args.config.is_none() {
        let written = modules::write_examples(&paths.config_file())
            .with_context(|| format!("cannot write {}", paths.config_file().display()))?;
        for file in written {
            tracing::info!(file = %file.display(), "wrote an example to start from");
        }
    }
    let config_file = args.config.unwrap_or_else(|| paths.config_file());
    let (store, loaded) = Store::load(&config_file, args.modules)?;

    // Claim the socket before touching anything a running daemon uses.
    std::fs::create_dir_all(&paths.runtime_dir)
        .with_context(|| format!("cannot create {}", paths.runtime_dir.display()))?;
    let listener = ipc::bind(&paths.socket())
        .await
        .with_context(|| format!("cannot listen on {}", paths.socket().display()))?;

    supervisor::check_version(&args.quickshell)?;

    let (connection_sender, connections) = mpsc::unbounded_channel();
    let (request_sender, requests) = mpsc::unbounded_channel();
    let (exit_sender, exits) = mpsc::unbounded_channel();
    let (ui_sender, ui) = mpsc::unbounded_channel();

    // Without a supported compositor this logs why and returns a handle whose
    // state says so; modules that need it stay idle.
    let compositor = mochi_core::compositor::connect();
    let mode = if args.dev { Mode::Link } else { Mode::Copy };
    let socket = paths.socket();
    let shell_dir = paths.shell_dir();
    let mut runner = Runner::new(paths, mode, compositor, request_sender, exit_sender);
    if let Some(dir) = config_file.parent() {
        runner.config_dir = dir.to_owned();
    }

    let files = Files {
        config: config_file,
    };
    let config = loaded.config.clone();
    let mut daemon = Daemon::new(runner, files, store, loaded);
    daemon.apply(&config)?;
    tracing::info!(modules = ?config.modules, "started modules");

    ipc::accept_all(listener, connection_sender);
    daemon.attach(Supervisor::spawn(
        UiCommand {
            program: args.quickshell,
            shell_dir,
            socket: socket.clone(),
        },
        ui_sender,
    )?);

    let result = daemon
        .run(Inputs {
            connections,
            requests,
            exits,
            ui,
        })
        .await;

    let _ = std::fs::remove_file(socket);
    result
}

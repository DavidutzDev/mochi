//! `mochi`, the command-line client for `mochid`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use mochi_protocol::{API, ActionSpec, ArgKind, ClientMessage, DaemonMessage, ModuleActions, Role};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run a module action, or list them.
    ///
    /// `mochi ipc` and `mochi ipc list` list every module's actions,
    /// `mochi ipc <module>` lists one module's, and
    /// `mochi ipc <module> <action> [args...]` runs one.
    Ipc {
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "MODULE ACTION ARGS"
        )]
        words: Vec<String>,
    },
    /// Show whether the daemon and the UI are running.
    Status,
    /// Apply changes to config.toml and theme.toml without a restart.
    Reload,
    /// Pick what to share, for xdg-desktop-portal-hyprland.
    ///
    /// Set `custom_picker_binary` in `~/.config/hypr/xdph.conf` to a program
    /// that runs this. It asks the share module and prints the portal's
    /// answer; when Mochi can't answer, it runs `hyprland-share-picker`
    /// instead, so sharing still works.
    SharePick {
        /// Let the app keep the choice by default.
        #[arg(long)]
        allow_token: bool,
    },
    /// Work with config.toml and theme.toml: `init`, `check`, `path`.
    ///
    /// Runs `mochid config`, which knows every module's settings, so it
    /// works without a running daemon.
    Config {
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "ARGS"
        )]
        args: Vec<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("mochi: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Ipc { words } => match words.as_slice() {
            [] => list(None),
            [only] if only == "list" => list(None),
            [module] => list(Some(module.clone())),
            [module, action, args @ ..] => {
                let answer = request(ClientMessage::Command {
                    module: module.clone(),
                    action: action.clone(),
                    args: args.to_vec(),
                })?;
                if let DaemonMessage::Output { output } = answer {
                    println!("{output}");
                }
                Ok(())
            }
        },
        Command::Status => match request(ClientMessage::Status)? {
            DaemonMessage::Status { status } => {
                println!("mochid {} (api {})", status.version, status.api);
                let ui = if status.ui_connected {
                    "connected"
                } else {
                    "not connected"
                };
                println!("ui:         {ui}");
                println!("modules:    {}", status.modules.join(", "));
                let compositor = &status.compositor;
                if compositor.backend == "unsupported" {
                    println!("compositor: no workspace information");
                } else {
                    let focused = compositor
                        .focused
                        .as_deref()
                        .map(|output| format!(", focus on {output}"))
                        .unwrap_or_default();
                    println!(
                        "compositor: {}, outputs {}, {} workspaces{focused}",
                        compositor.backend,
                        compositor.outputs.join(" "),
                        compositor.workspaces
                    );
                }
                Ok(())
            }
            other => Err(unexpected(&other)),
        },
        Command::Reload => {
            request(ClientMessage::Reload)?;
            println!("reloaded config.toml and theme.toml");
            Ok(())
        }
        Command::Config { args } => config(&args),
        Command::SharePick { allow_token } => share_pick(allow_token),
    }
}

/// Asks the share module what to share and prints its answer for the
/// portal. Falls back to Hyprland's own picker on any failure.
fn share_pick(allow_token: bool) -> Result<(), String> {
    let windows = std::env::var("XDPH_WINDOW_SHARING_LIST").unwrap_or_default();
    let mut args = vec![if allow_token { "on" } else { "off" }.to_owned()];
    if !windows.is_empty() {
        args.push(windows);
    }
    let answer = request(ClientMessage::Command {
        module: "share".into(),
        action: "pick".into(),
        args,
    });
    match answer {
        Ok(DaemonMessage::Output { output }) => {
            if !output.is_empty() {
                print!("{output}");
                if !output.ends_with('\n') {
                    println!();
                }
            }
            Ok(())
        }
        Ok(DaemonMessage::Ok) => Ok(()),
        Ok(other) => Err(unexpected(&other)),
        Err(message) => {
            eprintln!("mochi: {message}; using hyprland-share-picker");
            let mut picker = std::process::Command::new("hyprland-share-picker");
            if allow_token {
                picker.arg("--allow-token");
            }
            // Replaces this process, so the portal reads the picker's answer.
            let error = std::os::unix::process::CommandExt::exec(&mut picker);
            Err(format!("cannot start hyprland-share-picker: {error}"))
        }
    }
}

/// Hands over to `mochid config`: the one next to this binary when there
/// is one, so both come from the same build, otherwise the one in `PATH`.
fn config(args: &[String]) -> Result<(), String> {
    use std::os::unix::process::CommandExt;

    let sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("mochid")))
        .filter(|path| path.is_file());
    let program = sibling.map_or_else(|| "mochid".into(), PathBuf::into_os_string);
    let error = std::process::Command::new(&program)
        .arg("config")
        .args(args)
        .exec();
    Err(format!("cannot run {}: {error}", program.to_string_lossy()))
}

fn list(module: Option<String>) -> Result<(), String> {
    let DaemonMessage::Actions { modules } = request(ClientMessage::ListActions { module })? else {
        return Err("the daemon sent an unexpected answer".into());
    };
    for (index, ModuleActions { module, actions }) in modules.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{module}");
        if actions.is_empty() {
            println!("  (no actions)");
        }
        for action in actions {
            print_action(action);
        }
    }
    Ok(())
}

fn print_action(action: &ActionSpec) {
    println!("  {:<28} {}", action.usage(), action.help);
    for arg in &action.args {
        let kind = match &arg.kind {
            ArgKind::String => "text".to_owned(),
            ArgKind::Int => "integer".to_owned(),
            ArgKind::Float => "number".to_owned(),
            ArgKind::Bool => "on or off".to_owned(),
            ArgKind::Choice { values } => values.join(" | "),
        };
        println!("  {:<28}   {}: {} ({kind})", "", arg.name, arg.help);
    }
}

/// Sends one request and returns the answer. An `error` answer becomes `Err`.
fn request(message: ClientMessage) -> Result<DaemonMessage, String> {
    let path = socket()?;
    let mut stream = UnixStream::connect(&path).map_err(|error| {
        format!(
            "cannot reach mochid at {}: {error}\nIs it running? Start it with `mochid`.",
            path.display()
        )
    })?;

    let hello = ClientMessage::Hello {
        api: API,
        role: Role::Ctl,
    };
    for message in [&hello, &message] {
        let line = mochi_protocol::encode(message).map_err(|error| error.to_string())?;
        stream.write_all(&line).map_err(|error| error.to_string())?;
    }

    let mut lines = BufReader::new(stream).lines();
    loop {
        let line = lines
            .next()
            .ok_or("mochid closed the connection without answering")?
            .map_err(|error| error.to_string())?;
        match mochi_protocol::decode(&line).map_err(|error| error.to_string())? {
            DaemonMessage::Hello { .. } => continue,
            DaemonMessage::Error { message, .. } => return Err(message),
            answer => return Ok(answer),
        }
    }
}

fn socket() -> Result<PathBuf, String> {
    mochi_protocol::socket_path().ok_or_else(|| {
        format!(
            "cannot find the socket: set {} or XDG_RUNTIME_DIR",
            mochi_protocol::SOCKET_ENV
        )
    })
}

fn unexpected(message: &DaemonMessage) -> String {
    format!("the daemon sent an unexpected answer: {message:?}")
}

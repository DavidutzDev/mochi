//! `mochi`, the command-line client for `mochid`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::{BufRead, BufReader, IsTerminal, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use mochi_plugins::install::{Installer, Mode, Outcome, Plan};
use mochi_plugins::{Locations, Lock, PluginList};
use mochi_protocol::{
    API, ActionSpec, ArgKind, ClientMessage, DaemonMessage, ModuleActions, PluginState, Role,
};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Print JSON instead of text, for scripts: from `status`, `ipc list`,
    /// `ipc <module>` and `plugins list`.
    #[arg(long, global = true)]
    json: bool,
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
    /// Close what the island shows, like Escape or a right click on it.
    ///
    /// Bind it to a key: notices that don't take the keyboard, like the
    /// volume, can't hear Escape.
    Dismiss,
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
    /// Install, update and remove the plugins plugins.toml lists.
    ///
    /// plugins.toml sits next to config.toml. `install` and `update` show
    /// what each plugin will run and ask first; mochid reloads after.
    Plugins {
        /// The config.toml whose plugins.toml to use.
        #[arg(long, value_name = "FILE", global = true)]
        config: Option<PathBuf>,
        #[command(subcommand)]
        action: PluginsAction,
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
    /// Share setups, themes and plugins, and install them: `share`, `add`,
    /// `try`, `remove`, `list`, `check`.
    ///
    /// Runs `mochid bento`, which knows every module's settings, so it
    /// can tell what belongs to this machine.
    Bento {
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "ARGS"
        )]
        args: Vec<String>,
    },
    /// Check what Mochi needs around it: the daemon, the config,
    /// Quickshell, the fonts, the compositor's protocols, the portals and
    /// the programs modules and plugins run. Says what to install or
    /// change, and fails when something is broken.
    ///
    /// Runs `mochid doctor`.
    Doctor {
        /// The config.toml to check.
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
    /// Open a mochi:// link, as a browser does through the desktop entry.
    ///
    /// `mochi://bento/<source>` opens the settings' Bento page on what
    /// installing it would do, without installing anything: an id in the
    /// registry, or a repository like `github.com/someone/cozy`.
    OpenUrl { url: String },
    /// Print a completion script for a shell.
    ///
    /// For example, `mochi completions fish > ~/.config/fish/completions/mochi.fish`.
    Completions { shell: Shell },
}

#[derive(Debug, Subcommand)]
enum PluginsAction {
    /// Show each plugin, where it's from, and whether it runs.
    List,
    /// Install plugins that aren't yet, as plugins.lock pins them.
    Install {
        /// Only these plugins; all of them without any.
        ids: Vec<String>,
        /// Don't ask.
        #[arg(long, short)]
        yes: bool,
    },
    /// Fetch the latest of each plugin's branch or release, rebuild, and
    /// move plugins.lock.
    Update {
        /// Only these plugins; all of them without any.
        ids: Vec<String>,
        /// Don't ask.
        #[arg(long, short)]
        yes: bool,
    },
    /// Delete an installed plugin and its entry in plugins.lock.
    Remove { id: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command, cli.json) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("mochi: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command, json: bool) -> Result<(), String> {
    match command {
        Command::Ipc { words } => match words.as_slice() {
            [] => list(None, json),
            [only] if only == "list" => list(None, json),
            [module] => list(Some(module.clone()), json),
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
            DaemonMessage::Status { status } if json => print_json(&status),
            DaemonMessage::Status { status } => {
                println!("mochid {} (api {})", status.version, status.api);
                let ui = if status.ui_connected {
                    "connected"
                } else {
                    "not connected"
                };
                println!("ui:         {ui}");
                println!("modules:    {}", status.modules.join(", "));
                for plugin in &status.plugins {
                    let state = match plugin.state {
                        PluginState::Running => "running",
                        PluginState::Disabled => "disabled",
                        PluginState::Missing => "missing",
                        PluginState::Failed => "failed",
                    };
                    let note = plugin
                        .message
                        .as_deref()
                        .map(|message| format!(": {message}"))
                        .unwrap_or_default();
                    println!("plugin:     {} {state}{note}", plugin.id);
                }
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
        Command::Dismiss => request(ClientMessage::Dismiss).map(drop),
        Command::Reload => {
            request(ClientMessage::Reload)?;
            println!("reloaded config.toml and theme.toml");
            Ok(())
        }
        Command::Config { args } => mochid(&["config".into()], &args),
        Command::Bento { args } => mochid(&["bento".into()], &args),
        Command::OpenUrl { url } => open_url(&url),
        Command::Doctor { config } => {
            let mut before: Vec<String> = Vec::new();
            if let Some(config) = config {
                before.push("--config".into());
                before.push(config.to_string_lossy().into_owned());
            }
            before.push("doctor".into());
            mochid(&before, &[])
        }
        Command::Plugins { config, action } => plugins(config, action, json),
        Command::SharePick { allow_token } => share_pick(allow_token),
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "mochi", &mut std::io::stdout());
            Ok(())
        }
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    println!("{text}");
    Ok(())
}

/// `$XDG_CONFIG_HOME/mochi/config.toml`, like mochid's default.
fn default_config() -> Result<PathBuf, String> {
    let set = |name| std::env::var_os(name).filter(|value| !value.is_empty());
    let dir = set("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| set("HOME").map(|home| Path::new(&home).join(".config")))
        .ok_or("cannot find the config directory: set XDG_CONFIG_HOME or HOME")?;
    Ok(dir.join("mochi").join("config.toml"))
}

fn plugins(config: Option<PathBuf>, action: PluginsAction, json: bool) -> Result<(), String> {
    let config = match config {
        Some(config) => config,
        None => default_config()?,
    };
    let locations = Locations::beside(&config);
    let list = PluginList::of(&locations).map_err(|error| error.to_string())?;
    match action {
        PluginsAction::List => plugins_list(&locations, &list, json),
        PluginsAction::Install { ids, yes } => {
            plugins_install(&locations, &list, &ids, yes, Mode::Install)
        }
        PluginsAction::Update { ids, yes } => {
            plugins_install(&locations, &list, &ids, yes, Mode::Update)
        }
        PluginsAction::Remove { id } => {
            let mut confirm = |_: &Plan| true;
            let installer = Installer {
                locations: &locations,
                confirm: &mut confirm,
            };
            if installer.remove(&id).map_err(|error| error.to_string())? {
                println!("removed {id}");
                reload_if_running();
            } else {
                println!("{id} wasn't installed");
            }
            if list.plugins.contains_key(&id) {
                println!(
                    "{id} is still in {}: remove it there too, or `mochi plugins install` brings it back",
                    locations.list.display()
                );
            }
            Ok(())
        }
    }
}

fn plugins_list(locations: &Locations, list: &PluginList, json: bool) -> Result<(), String> {
    if list.plugins.is_empty() && json {
        return print_json(&[(); 0]);
    }
    if list.plugins.is_empty() {
        println!("no plugins in {}", locations.list.display());
        return Ok(());
    }
    let lock = Lock::load(&locations.lock).map_err(|error| error.to_string())?;
    // What mochid says, when it runs.
    let running = match request(ClientMessage::Status) {
        Ok(DaemonMessage::Status { status }) => Some(status.plugins),
        _ => None,
    };
    let found = mochi_plugins::discover(locations).map_err(|error| error.to_string())?;
    let mut entries = Vec::new();
    for plugin in found {
        let locked = lock.plugins.get(&plugin.id);
        let revision = locked
            .and_then(|locked| locked.revision())
            .map(|revision| format!(" at {revision}"))
            .unwrap_or_default();
        let version = plugin
            .manifest
            .as_ref()
            .map(|manifest| format!(" {}", manifest.plugin.version))
            .unwrap_or_default();
        let daemon = running
            .as_ref()
            .and_then(|plugins| plugins.iter().find(|status| status.id == plugin.id));
        let state = match (daemon, &plugin.manifest) {
            (Some(status), _) => match status.state {
                PluginState::Running => "running",
                PluginState::Disabled => "disabled",
                PluginState::Missing => "missing",
                PluginState::Failed => "failed",
            },
            (None, Ok(_)) => "installed",
            (None, Err(_)) => "missing",
        };
        let note = daemon
            .and_then(|status| status.message.clone())
            .or_else(|| plugin.manifest.as_ref().err().cloned());
        if json {
            entries.push(serde_json::json!({
                "id": plugin.id,
                "source": plugin.source.to_string(),
                "state": state,
                "revision": locked.and_then(|locked| locked.revision()),
                "version": plugin.manifest.as_ref().ok().map(|manifest| &manifest.plugin.version),
                "message": note,
            }));
            continue;
        }
        println!(
            "{:<16} {state:<9} {}{revision}{version}",
            plugin.id, plugin.source
        );
        if let Some(note) = note {
            println!("{:<16} {note}", "");
        }
    }
    if json {
        return print_json(&entries);
    }
    if running.is_none() {
        println!("(mochid isn't running, so this doesn't say which run)");
    }
    Ok(())
}

fn plugins_install(
    locations: &Locations,
    list: &PluginList,
    ids: &[String],
    yes: bool,
    mode: Mode,
) -> Result<(), String> {
    for id in ids {
        if !list.plugins.contains_key(id) {
            return Err(format!(
                "{id} isn't in {}: add it there first, like\n\n[plugins.{id}]\nsource = \"git:github.com/<user>/<repo>\"",
                locations.list.display()
            ));
        }
    }
    if list.plugins.is_empty() {
        println!("no plugins in {}", locations.list.display());
        return Ok(());
    }
    if !yes && !std::io::stdin().is_terminal() {
        return Err("not on a terminal, so nobody can confirm: pass --yes".into());
    }
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
        locations,
        confirm: &mut confirm,
    };
    let mut changed = false;
    let mut failed = 0;
    for (id, source) in &list.plugins {
        if !ids.is_empty() && !ids.contains(id) {
            continue;
        }
        match installer.run(id, source, mode) {
            Ok(Outcome::Installed { revision }) => {
                changed = true;
                let at = revision
                    .map(|revision| format!(" at {revision}"))
                    .unwrap_or_default();
                println!("installed {id}{at}");
            }
            Ok(Outcome::UpToDate { revision }) => {
                let at = revision
                    .map(|revision| format!(" at {revision}"))
                    .unwrap_or_default();
                println!("{id} is up to date{at}");
            }
            Ok(Outcome::Declined) => println!("skipped {id}"),
            Err(error) => {
                failed += 1;
                eprintln!("mochi: {id}: {error}");
            }
        }
    }
    if changed {
        reload_if_running();
    }
    match failed {
        0 => Ok(()),
        1 => Err("one plugin failed".into()),
        count => Err(format!("{count} plugins failed")),
    }
}

/// Tells a running mochid to pick up the change; without one, the next
/// start does.
fn reload_if_running() {
    match request(ClientMessage::Reload) {
        Ok(_) => println!("mochid reloaded"),
        Err(error) if error.starts_with("cannot reach mochid") => {}
        Err(error) => eprintln!("mochi: mochid didn't reload: {error}"),
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
/// Runs mochid, the one next to this program when there is one, with
/// `before` then `args`.
fn mochid(before: &[String], args: &[String]) -> Result<(), String> {
    use std::os::unix::process::CommandExt;

    let sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("mochid")))
        .filter(|path| path.is_file());
    let program = sibling.map_or_else(|| "mochid".into(), PathBuf::into_os_string);
    let error = std::process::Command::new(&program)
        .args(before)
        .args(args)
        .exec();
    Err(format!("cannot run {}: {error}", program.to_string_lossy()))
}

fn list(module: Option<String>, json: bool) -> Result<(), String> {
    let DaemonMessage::Actions { modules } = request(ClientMessage::ListActions { module })? else {
        return Err("the daemon sent an unexpected answer".into());
    };
    if json {
        return print_json(&modules);
    }
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

/// `mochi://bento/<source>`, or `mochi://bento/add/<source>`: the Bento
/// page on that source, which shows what installing it does and asks.
fn open_url(url: &str) -> Result<(), String> {
    let rest = url
        .strip_prefix("mochi://")
        .ok_or_else(|| format!("{url:?} isn't a mochi:// link"))?;
    let source = rest
        .strip_prefix("bento/add/")
        .or_else(|| rest.strip_prefix("bento/"))
        .map(decode)
        .filter(|source| !source.trim().is_empty())
        .ok_or_else(|| format!("{url:?} names nothing Mochi opens: mochi://bento/<id> does"))?;
    match request(ClientMessage::Command {
        module: "settings".into(),
        action: "bento-show".into(),
        args: vec![source],
    })? {
        DaemonMessage::Ok | DaemonMessage::Output { .. } => Ok(()),
        other => Err(unexpected(&other)),
    }
}

/// `%2F` and the like back into what they stand for.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                index += 3;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out)
        .trim_end_matches('/')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_decode() {
        assert_eq!(decode("cozy"), "cozy");
        assert_eq!(
            decode("github.com%2Fsomeone%2Fcozy/"),
            "github.com/someone/cozy"
        );
        assert_eq!(decode("100%"), "100%");
    }
}

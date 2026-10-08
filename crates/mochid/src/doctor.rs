//! `mochid doctor`, which `mochi doctor` runs: checks what Mochi needs
//! around it and says what to install or change. Each line is ✓ fine,
//! ! something works less well, or ✗ broken; the command fails when a
//! line is ✗.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mochi_core::Paths;
use mochi_protocol::{API, ClientMessage, DaemonMessage, Role};

use crate::modules;
use crate::settings::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Ok,
    Warn,
    Fail,
}

/// What the checks found, section by section.
#[derive(Debug, Default)]
struct Report {
    lines: Vec<String>,
    worst: Option<Level>,
}

impl Report {
    fn section(&mut self, title: &str) {
        if !self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.lines.push(title.to_owned());
    }

    fn add(&mut self, level: Level, text: impl AsRef<str>, hint: Option<&str>) {
        let mark = match level {
            Level::Ok => "✓",
            Level::Warn => "!",
            Level::Fail => "✗",
        };
        self.lines.push(format!("  {mark} {}", text.as_ref()));
        if let Some(hint) = hint {
            self.lines.push(format!("    {hint}"));
        }
        self.worst = self.worst.max(Some(level));
    }

    fn ok(&mut self, text: impl AsRef<str>) {
        self.add(Level::Ok, text, None);
    }

    fn warn(&mut self, text: impl AsRef<str>, hint: &str) {
        self.add(Level::Warn, text, Some(hint));
    }

    fn fail(&mut self, text: impl AsRef<str>, hint: &str) {
        self.add(Level::Fail, text, Some(hint));
    }
}

/// The protocols Mochi looks for, any of a group doing, and what they're
/// for. The first is required: without it there is no island.
const PROTOCOLS: [(&[&str], &str, bool); 8] = [
    (
        &["zwlr_layer_shell_v1"],
        "the island, the bubbles and the widgets",
        true,
    ),
    (&["ext_workspace_manager_v1"], "workspaces", false),
    (
        &["zwlr_foreign_toplevel_manager_v1"],
        "which monitor and app have the focus",
        false,
    ),
    (
        &[
            "zwlr_screencopy_manager_v1",
            "ext_image_copy_capture_manager_v1",
        ],
        "screenshots, the color picker and live previews",
        false,
    ),
    (
        &["ext_data_control_manager_v1"],
        "the clipboard history",
        false,
    ),
    (
        &["ext_background_effect_manager_v1"],
        "blur behind the island",
        false,
    ),
    (&["zwlr_gamma_control_manager_v1"], "night light", false),
    (
        &["zwlr_virtual_pointer_manager_v1"],
        "passing on the click that closes a panel, so one click does",
        false,
    ),
];

/// Runs every check and prints the report. Fails when something is broken.
#[allow(clippy::print_stdout)]
pub async fn run(config_file: &Path) -> anyhow::Result<bool> {
    let mut report = Report::default();
    report
        .lines
        .push(format!("Mochi {}", env!("CARGO_PKG_VERSION")));
    daemon(&mut report);
    let config = config(&mut report, config_file);
    quickshell(&mut report);
    fonts(&mut report);
    compositor(&mut report);
    portals(&mut report).await;
    if config
        .as_ref()
        .is_some_and(|config| config.modules.iter().any(|id| id == "notifications"))
    {
        notifications(&mut report).await;
    }
    if let Some(config) = &config {
        needs(&mut report, config, config_file);
    }
    for line in &report.lines {
        println!("{line}");
    }
    Ok(report.worst != Some(Level::Fail))
}

/// Whether mochid answers on its socket, and the UI is connected.
fn daemon(report: &mut Report) {
    report.section("Daemon");
    let socket = std::env::var_os("MOCHI_SOCKET")
        .map(PathBuf::from)
        .or_else(|| Paths::from_env().ok().map(|paths| paths.socket()));
    let Some(socket) = socket else {
        report.fail(
            "XDG_RUNTIME_DIR isn't set",
            "Run this inside your graphical session.",
        );
        return;
    };
    match status(&socket) {
        Ok(status) if status.ui_connected => report.ok(format!(
            "mochid {} runs {} modules, and the UI is connected",
            status.version,
            status.modules.len()
        )),
        Ok(status) => report.fail(
            format!("mochid {} runs, but the UI isn't connected", status.version),
            "Quickshell isn't up: `journalctl --user -u mochid` says why.",
        ),
        Err(_) => report.warn(
            "mochid isn't running",
            "Start it with `systemctl --user start mochid`, or run `mochid`.",
        ),
    }
}

fn status(socket: &Path) -> std::io::Result<mochi_protocol::Status> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut send = |message: &ClientMessage| -> std::io::Result<()> {
        let mut line = serde_json::to_vec(message)?;
        line.push(b'\n');
        stream.write_all(&line)
    };
    send(&ClientMessage::Hello {
        api: API,
        role: Role::Ctl,
    })?;
    send(&ClientMessage::Status)?;
    let reader = BufReader::new(stream.try_clone()?);
    for line in reader.lines() {
        if let Ok(DaemonMessage::Status { status }) = serde_json::from_str(&line?) {
            return Ok(status);
        }
    }
    Err(std::io::Error::other("no status"))
}

fn config(report: &mut Report, config_file: &Path) -> Option<mochi_core::Config> {
    report.section("Configuration");
    match Store::load(config_file, None) {
        Ok((store, loaded)) => {
            report.ok(format!("{} and theme.toml are fine", config_file.display()));
            if store.has_changes() {
                report.warn(
                    "the settings panel has changes over them, in changes.toml",
                    "Copy them into your files from Settings, or drop them with Undo all changes.",
                );
            }
            Some(loaded.config)
        }
        Err(error) => {
            report.fail(
                error.to_string(),
                "Fix the file; `mochi config check` says the same.",
            );
            None
        }
    }
}

fn quickshell(report: &mut Report) {
    report.section("Quickshell");
    let program: OsString = "quickshell".into();
    match mochi_core::supervisor::check_version(&program) {
        Ok(()) => report.ok(format!("quickshell {}", version(&program))),
        Err(error) => report.fail(
            error.to_string(),
            "Install Quickshell 0.3: the Nix and Arch packages bring it.",
        ),
    }
}

fn version(program: &OsString) -> String {
    std::process::Command::new(program)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .nth(1)
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

fn fonts(report: &mut Report) {
    report.section("Fonts");
    let found = mochi_core::assets::find_fonts();
    let has = |part: &str| {
        found.iter().any(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().contains(part))
        })
    };
    if has("Inter") {
        report.ok("Inter, for text");
    } else {
        report.warn(
            "Inter isn't found: text uses the system font",
            "Install Inter, or point MOCHI_FONTS at a folder with it.",
        );
    }
    if has("MaterialSymbols") {
        report.ok("Material Symbols Rounded, for icons");
    } else {
        report.warn(
            "Material Symbols Rounded isn't found: icons fall back to simpler drawn ones",
            "Install it, or point MOCHI_FONTS at a folder with it.",
        );
    }
}

fn compositor(report: &mut Report) {
    report.section("Compositor");
    let running = mochi_core::compositor::running();
    match running {
        Some("Hyprland") => report.ok(
            "Hyprland: the focus, windows, what's shared, and switchable shares",
        ),
        Some("niri") => {
            report.ok("niri: the focus, windows and what's shared");
            report.warn(
                "switchable shares need Hyprland",
                "On niri, a share is the screen, window or area you pick; niri's own dynamic cast target switches too.",
            );
        }
        Some("sway") => {
            report.ok("Sway: the focus and windows");
            report.warn(
                "Sway doesn't say what's shared",
                "No Sharing bubble, and switchable shares need Hyprland.",
            );
        }
        _ => report.warn(
            "a compositor Mochi has no IPC for",
            "Workspaces and the focus come from Wayland protocols only, where it has them; windows and what's shared are unknown.",
        ),
    }
    let globals: BTreeSet<String> = match mochi_core::compositor::globals() {
        Ok(globals) => globals.into_iter().collect(),
        Err(error) => {
            report.fail(error, "Run this inside your Wayland session.");
            return;
        }
    };
    for (names, purpose, required) in PROTOCOLS {
        let found = names.iter().find(|name| globals.contains(**name));
        match (found, required) {
            (Some(name), _) => report.ok(format!("{name}: {purpose}")),
            (None, true) => report.fail(
                format!("no {}: {purpose} can't show", names[0]),
                "Mochi needs a compositor with wlr-layer-shell, like Hyprland, niri or Sway.",
            ),
            (None, false) => report.warn(
                format!("no {}: no {purpose}", names.join(" or ")),
                if names[0] == "ext_workspace_manager_v1" && running == Some("sway") {
                    "Sway has it from 1.12."
                } else {
                    "Your compositor doesn't offer it; the rest works."
                },
            ),
        }
    }
}

/// The portal, and on Hyprland, Mochi's picker for its screen sharing.
async fn portals(report: &mut Report) {
    report.section("Portals");
    let Ok(bus) = zbus::Connection::session().await else {
        report.fail(
            "no D-Bus session bus",
            "Run this inside your graphical session.",
        );
        return;
    };
    let names = dbus_names(&bus).await;
    if names
        .iter()
        .any(|name| name == "org.freedesktop.portal.Desktop")
    {
        report.ok("xdg-desktop-portal, for screen sharing and file pickers");
    } else {
        report.warn(
            "xdg-desktop-portal isn't running or installed",
            "Apps can't share the screen without it.",
        );
    }
    if mochi_core::compositor::running() != Some("Hyprland") {
        return;
    }
    let portal = data_dirs().into_iter().any(|dir| {
        dir.join("xdg-desktop-portal/portals/hyprland.portal")
            .is_file()
    });
    if !portal {
        report.warn(
            "xdg-desktop-portal-hyprland isn't installed",
            "Install it to share the screen with Mochi's picker.",
        );
        return;
    }
    let config = config_home().map(|home| home.join("hypr/xdph.conf"));
    let picker = config
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|text| text.contains("mochi-share-picker") || text.contains("share-pick"));
    if picker {
        report.ok("xdg-desktop-portal-hyprland, with Mochi's share picker");
    } else {
        report.warn(
            "xdg-desktop-portal-hyprland uses its own picker",
            "Set `custom_picker_binary = mochi-share-picker` under `screencopy` in ~/.config/hypr/xdph.conf; home-manager's portalPicker does it.",
        );
    }
}

/// Another notification daemon holds the name, so Mochi's waits.
async fn notifications(report: &mut Report) {
    report.section("Notifications");
    let Ok(bus) = zbus::Connection::session().await else {
        return;
    };
    let Ok(proxy) = zbus::fdo::DBusProxy::new(&bus).await else {
        return;
    };
    let name = "org.freedesktop.Notifications";
    let Ok(owner) = proxy
        .get_name_owner(zbus::names::BusName::try_from(name).expect("a valid name"))
        .await
    else {
        report.warn(
            "nobody shows notifications",
            "mochid isn't running, or its notifications module didn't start.",
        );
        return;
    };
    let program = proxy
        .get_connection_unix_process_id(owner.into())
        .await
        .ok()
        .and_then(|pid| std::fs::read_to_string(format!("/proc/{pid}/comm")).ok())
        .map(|comm| comm.trim().to_owned())
        .unwrap_or_default();
    if program.starts_with("mochid") || program.starts_with(".mochid") {
        report.ok("Mochi shows notifications");
    } else {
        report.fail(
            format!("{program} holds {name}, so Mochi's notifications wait"),
            "Stop it and turn it off, like `systemctl --user disable --now dunst` or mako, swaync.",
        );
    }
}

async fn dbus_names(bus: &zbus::Connection) -> Vec<String> {
    let Ok(proxy) = zbus::fdo::DBusProxy::new(bus).await else {
        return Vec::new();
    };
    let mut names: Vec<String> = proxy
        .list_names()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|name| name.to_string())
        .collect();
    names.extend(
        proxy
            .list_activatable_names()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|name| name.to_string()),
    );
    names
}

/// The programs the enabled modules and plugins run.
fn needs(report: &mut Report, config: &mochi_core::Config, config_file: &Path) {
    report.section("Programs");
    let Ok(catalog) = modules::catalog(config_file) else {
        return;
    };
    let mut fine = Vec::new();
    for module in &catalog.modules {
        if !config.modules.iter().any(|id| id == module.id()) {
            continue;
        }
        let clipboard = config.modules.iter().any(|id| id == "clipboard");
        for need in module.needs(&config.settings(module.id())) {
            // Copying goes through the clipboard module when it runs.
            if clipboard && need.program == "wl-copy" && need.purpose.contains("clipboard module") {
                continue;
            }
            if mochi_plugins::on_path(&need.program) {
                fine.push(format!("{} ({})", need.program, module.id()));
            } else if need.required {
                report.fail(
                    format!("{}: {} isn't installed", module.id(), need.program),
                    &format!("{} needs it.", need.purpose),
                );
            } else {
                report.warn(
                    format!("{}: {} isn't installed", module.id(), need.program),
                    &format!("{} won't work without it.", need.purpose),
                );
            }
        }
    }
    for listed in &catalog.listed {
        if let Some(problem) = &listed.problem {
            report.fail(
                format!("plugin {}: {problem}", listed.id),
                "See `mochi plugins`.",
            );
        }
    }
    fine.sort();
    fine.dedup();
    if !fine.is_empty() {
        report.ok(fine.join(", "));
    }
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
}

fn data_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned())
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .collect();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(home));
    }
    dirs.push(PathBuf::from("/run/current-system/sw/share"));
    dirs
}

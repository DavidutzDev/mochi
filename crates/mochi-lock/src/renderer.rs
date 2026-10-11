//! The lock screen's renderer: a Quickshell instance that draws the lock
//! screen's views on the private display, and its control socket.
//!
//! The renderer can draw and gets the pointer, nothing else. It sees the
//! lock's state as a module state, `lock`: how many characters are typed,
//! not what they are. It gets the data mochid sends for the widgets it may
//! show, and the theme. It can ask for a few commands, like play and
//! pause, which the lock checks against [`ALLOWED`] before passing them
//! on, and mochid checks again. It runs with a scrubbed environment, so it
//! doesn't find the real display or the compositor's IPC by the usual
//! names.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::config::Renderer as Settings;
use crate::display::{Private, peer};

/// The commands the renderer may ask for: module and action.
pub const ALLOWED: [(&str, &str); 3] = [
    ("media", "play-pause"),
    ("media", "next"),
    ("media", "previous"),
];

/// The environment the renderer keeps from the lock's; everything else,
/// like `WAYLAND_DISPLAY` and `HYPRLAND_INSTANCE_SIGNATURE`, goes.
const KEPT: [&str; 16] = [
    "HOME",
    "USER",
    "LOGNAME",
    "PATH",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_TIME",
    "LC_MESSAGES",
    "TZ",
    "TZDIR",
    "XDG_RUNTIME_DIR",
    "XDG_DATA_DIRS",
    "FONTCONFIG_FILE",
    "FONTCONFIG_PATH",
    "LOCALE_ARCHIVE",
];

/// A command the renderer asked for and [`ALLOWED`] lets through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub module: String,
    pub action: String,
}

pub struct Renderer {
    settings: Settings,
    /// A directory only the lock uses, for its two sockets.
    dir: PathBuf,
    pub display: Private,
    control: UnixListener,
    client: Option<Talking>,
    child: Option<Child>,
    /// What the renderer gets on connecting, and again on every change:
    /// the theme, and module states by module.
    theme: Value,
    states: BTreeMap<String, Value>,
    started: Option<Instant>,
    /// The monitors the renderer said it has drawn as they should be.
    ready: std::collections::BTreeSet<String>,
    /// Starts in a row that ended before drawing anything.
    failures: u32,
    restart_at: Option<Instant>,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Renderer")
            .field("dir", &self.dir)
            .field("running", &self.child.is_some())
            .finish_non_exhaustive()
    }
}

struct Talking {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Renderer {
    /// Opens the private display and the control socket in a directory of
    /// their own, and starts the renderer.
    pub fn new(settings: Settings, theme: Value) -> Result<Self, String> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .ok_or("XDG_RUNTIME_DIR isn't set")?;
        sweep(&runtime);
        let dir = runtime.join(format!("mochi-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&dir)
                .map_err(|error| format!("can't make {}: {error}", dir.display()))?;
        }
        let display = Private::new(&dir.join("display"))?;
        let control = UnixListener::bind(dir.join("control"))
            .map_err(|error| format!("can't listen for the renderer: {error}"))?;
        control
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let mut renderer = Self {
            settings,
            dir,
            display,
            control,
            client: None,
            child: None,
            theme,
            states: BTreeMap::new(),
            started: None,
            ready: std::collections::BTreeSet::new(),
            failures: 0,
            restart_at: None,
        };
        renderer.start();
        Ok(renderer)
    }

    fn start(&mut self) {
        self.restart_at = None;
        self.client = None;
        self.ready.clear();
        self.display.server.frames.clear();
        let mut command = Command::new(&self.settings.quickshell);
        command
            .arg("--path")
            .arg(&self.settings.shell)
            .arg("--no-color")
            .env_clear()
            .stdin(Stdio::null());
        // What it says goes to a log in the lock's directory, gone at unlock.
        match std::fs::File::create(self.dir.join("renderer.log")) {
            Ok(log) => {
                let copy = log.try_clone().map_or_else(|_| Stdio::null(), Stdio::from);
                command.stdout(copy).stderr(Stdio::from(log));
            }
            Err(_) => {
                command.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }
        for name in KEPT {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let nobus = format!("unix:path={}", self.dir.join("nobus").display());
        command
            .env("WAYLAND_DISPLAY", self.dir.join("display"))
            .env("QT_QPA_PLATFORM", "wayland")
            .env("QT_QUICK_BACKEND", "software")
            .env("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1")
            .env("MOCHI_SOCKET", self.dir.join("control"))
            .env("MOCHI_WATCH", "0")
            // No bus: the lock screen's views need none, and Qt would
            // start one of its own without a dead address.
            .env("DBUS_SESSION_BUS_ADDRESS", &nobus)
            .env("DBUS_SYSTEM_BUS_ADDRESS", &nobus);
        match command.spawn() {
            Ok(child) => {
                self.display.server.allow(Some(child.id()));
                self.child = Some(child);
                self.started = Some(Instant::now());
            }
            Err(error) => {
                crate::lock::report(
                    "warning",
                    Some(&format!("can't start the lock screen's renderer: {error}")),
                );
                self.failures += 1;
                self.schedule();
            }
        }
    }

    /// Whether it said a monitor is drawn as it should be.
    pub fn is_ready(&self, output: &str) -> bool {
        self.ready.contains(output)
    }

    /// When it started, while it runs.
    pub fn started(&self) -> Option<Instant> {
        self.child.as_ref().and(self.started)
    }

    fn schedule(&mut self) {
        // 1, 2, 4… seconds, up to a minute, after renderers that end
        // before drawing. Meanwhile the lock draws its own face.
        let wait = Duration::from_secs((1u64 << self.failures.min(6)).min(60));
        self.restart_at = Some(Instant::now() + wait);
    }

    /// Does what's due: takes the renderer's connection, handles what it
    /// and its clients sent, notices it ended. Returns the commands it
    /// asked for.
    pub fn poll(&mut self) -> Vec<Asked> {
        let mut asked = Vec::new();
        let _ = self.display.accept();
        self.display.dispatch();
        self.display.server.tick();

        while let Ok((stream, _)) = self.control.accept() {
            let pid = self.child.as_ref().map(Child::id);
            if pid.is_none() || peer(&stream) != pid {
                continue;
            }
            let Ok(writer) = stream.try_clone() else {
                continue;
            };
            let _ = stream.set_nonblocking(true);
            self.client = Some(Talking {
                reader: BufReader::new(stream),
                writer,
            });
            crate::lock::timing("renderer connected");
            self.greet();
        }
        if let Some(talking) = &mut self.client {
            let mut line = String::new();
            loop {
                line.clear();
                match talking.reader.read_line(&mut line) {
                    Ok(0) => {
                        self.client = None;
                        break;
                    }
                    Ok(_) => {
                        // Its own word that a monitor is drawn: for the lock, not
                        // a command for mochid.
                        if let Some(output) = ready(&line) {
                            crate::lock::timing(&format!("renderer ready on {output}"));
                            self.ready.insert(output);
                        } else {
                            asked.extend(allowed(&line));
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(_) => {
                        self.client = None;
                        break;
                    }
                }
            }
        }

        if let Some(child) = &mut self.child
            && let Ok(Some(_)) = child.try_wait()
        {
            let drew = !self.display.server.frames.is_empty();
            self.child = None;
            self.client = None;
            self.display.server.allow(None);
            self.ready.clear();
            self.display.server.frames.clear();
            self.failures = if drew { 0 } else { self.failures + 1 };
            crate::lock::report("warning", Some("the lock screen's renderer ended"));
            self.schedule();
        }
        if self.child.is_none() && self.restart_at.is_some_and(|at| Instant::now() >= at) {
            self.start();
        }
        asked
    }

    fn greet(&mut self) {
        let mut lines = vec![
            json!({ "type": "hello", "api": 1 }),
            json!({ "type": "theme", "theme": self.theme }),
            json!({ "type": "modules", "modules": self.states.keys().collect::<Vec<_>>() }),
        ];
        lines.extend(
            self.states.iter().map(
                |(module, state)| json!({ "type": "state", "module": module, "state": state }),
            ),
        );
        for line in lines {
            self.send(&line);
        }
    }

    fn send(&mut self, message: &Value) {
        let Some(talking) = &mut self.client else {
            return;
        };
        let line = format!("{message}\n");
        if talking.writer.write_all(line.as_bytes()).is_err() {
            self.client = None;
        }
    }

    pub fn theme(&mut self, theme: Value) {
        self.theme = theme;
        let message = json!({ "type": "theme", "theme": self.theme });
        self.send(&message);
    }

    /// A module's state, or the lock's own, `lock`.
    pub fn state(&mut self, module: &str, state: Value) {
        let known = self.states.contains_key(module);
        self.states.insert(module.to_owned(), state.clone());
        if !known {
            let modules =
                json!({ "type": "modules", "modules": self.states.keys().collect::<Vec<_>>() });
            self.send(&modules);
        }
        self.send(&json!({ "type": "state", "module": module, "state": state }));
    }

    /// Stops the renderer and removes its directory, once unlocked.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Removes the directories of locks that were killed before they could:
/// `mochi-lock-<pid>` whose process isn't a `mochi-lock` any more.
fn sweep(runtime: &Path) {
    let Ok(entries) = std::fs::read_dir(runtime) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|name| name.strip_prefix("mochi-lock-"))
            .and_then(|pid| pid.parse::<u32>().ok())
        else {
            continue;
        };
        let alive = std::fs::read_to_string(format!("/proc/{pid}/comm"))
            .is_ok_and(|comm| comm.trim() == "mochi-lock");
        if !alive {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The monitor the renderer says it has drawn, from its `lock ready`.
fn ready(line: &str) -> Option<String> {
    let message: Value = serde_json::from_str(line).ok()?;
    (message["type"] == "command" && message["module"] == "lock" && message["action"] == "ready")
        .then(|| message["args"][0].as_str().map(str::to_owned))
        .flatten()
}

/// A command from the renderer, if it's one it may ask for.
fn allowed(line: &str) -> Option<Asked> {
    let message: Value = serde_json::from_str(line).ok()?;
    if message["type"] != "command" {
        return None;
    }
    let (module, action) = (message["module"].as_str()?, message["action"].as_str()?);
    ALLOWED
        .iter()
        .any(|(allowed_module, allowed_action)| {
            *allowed_module == module && *allowed_action == action
        })
        .then(|| Asked {
            module: module.to_owned(),
            action: action.to_owned(),
        })
}

/// Whether `path` holds a shell for the renderer.
pub fn usable(settings: &Settings) -> bool {
    Path::new(&settings.shell).join("shell.qml").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_allowed_commands_get_through() {
        let line = |module: &str, action: &str| {
            json!({ "type": "command", "module": module, "action": action, "args": [] }).to_string()
        };
        assert_eq!(
            allowed(&line("media", "play-pause")),
            Some(Asked {
                module: "media".into(),
                action: "play-pause".into()
            })
        );
        assert_eq!(allowed(&line("launcher", "open")), None);
        assert_eq!(allowed(&line("power", "shutdown")), None);
        assert_eq!(allowed(&line("lock", "unlock")), None);
        assert_eq!(
            allowed(r#"{"type":"event","module":"media","action":"next"}"#),
            None
        );
        assert_eq!(allowed("not json"), None);
    }
}

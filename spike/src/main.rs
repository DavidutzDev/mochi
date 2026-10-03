//! Throwaway spike for Mochi. It answers the questions in docs/spike.md and
//! gets deleted once the real daemon exists.

mod assets;
mod ipc;
mod island;
mod protocol;
mod supervisor;

use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};
use std::{env, fs};

use ipc::ConnectionId;
use island::Island;
use protocol::{API, EventKind, Incoming, Outgoing, Role};
use supervisor::Supervisor;

const USAGE: &str = "usage:
  mochi-spike run [--dev] [--modules idle,demo]
  mochi-spike ipc <module> <action> [args...]

actions:
  idle show
  demo show <Small|Wide|Card|Big> [text...]";

/// How long Quickshell gets between starting and saying hello.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const DEMO_TIMEOUT: Duration = Duration::from_secs(4);

/// Everything the daemon loop reacts to.
pub enum Event {
    Connected {
        id: ConnectionId,
        writer: UnixStream,
    },
    Message {
        id: ConnectionId,
        message: Incoming,
    },
    Malformed {
        id: ConnectionId,
        error: String,
    },
    Disconnected {
        id: ConnectionId,
    },
    UiStarted,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("ipc") if args.len() >= 3 => runtime_dir()
            .and_then(|dir| ipc::call(&dir.join("mochi.sock"), &args[1], &args[2], &args[3..])),
        _ => Err(USAGE.to_owned()),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("mochi-spike: {message}");
            ExitCode::FAILURE
        }
    }
}

fn runtime_dir() -> Result<PathBuf, String> {
    env::var_os("XDG_RUNTIME_DIR")
        .map(|dir| PathBuf::from(dir).join("mochi-spike"))
        .ok_or_else(|| "XDG_RUNTIME_DIR is not set".to_owned())
}

fn run(args: &[String]) -> Result<(), String> {
    let mut mode = assets::Mode::Copy;
    let mut modules = vec!["idle".to_owned(), "demo".to_owned()];

    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dev" => mode = assets::Mode::Symlink,
            "--modules" => {
                let list = args
                    .next()
                    .ok_or("--modules needs a comma-separated list")?;
                modules = list.split(',').map(str::to_owned).collect();
            }
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    if !modules.iter().any(|module| module == "idle") {
        return Err("the idle module is required".into());
    }

    let root = runtime_dir()?;
    let shell_dir = root.join("shell");
    let socket = root.join("mochi.sock");
    fs::create_dir_all(&root)
        .map_err(|error| format!("cannot create {}: {error}", root.display()))?;

    // Listen first: it fails if another daemon is running, and that daemon's
    // shell directory must not be touched.
    let (sender, events) = mpsc::channel();
    ipc::listen(&socket, sender.clone()).map_err(|error| format!("cannot listen: {error}"))?;

    let written = assets::write_shell(&shell_dir, &modules, mode)
        .map_err(|error| format!("cannot write the shell: {error}"))?;
    eprintln!(
        "assets: {mode:?} mode, {written} entries written to {}",
        shell_dir.display()
    );

    let supervisor = supervisor::spawn(shell_dir, socket, sender)
        .map_err(|error| format!("cannot run quickshell: {error}"))?;

    Daemon::new(modules, supervisor).run(&events);
    Ok(())
}

struct Client {
    writer: UnixStream,
    role: Option<Role>,
}

struct Daemon {
    modules: Vec<String>,
    supervisor: Supervisor,
    clients: HashMap<ConnectionId, Client>,
    island: Island,
    handshake_deadline: Option<Instant>,
}

impl Daemon {
    fn new(modules: Vec<String>, supervisor: Supervisor) -> Self {
        Self {
            modules,
            supervisor,
            clients: HashMap::new(),
            island: Island::new(),
            handshake_deadline: None,
        }
    }

    fn run(mut self, events: &Receiver<Event>) {
        loop {
            let deadline = [self.island.deadline(), self.handshake_deadline]
                .into_iter()
                .flatten()
                .min();

            let event = match deadline {
                Some(deadline) => {
                    match events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                        Ok(event) => Some(event),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                None => match events.recv() {
                    Ok(event) => Some(event),
                    Err(_) => return,
                },
            };

            if let Some(event) = event {
                self.handle(event);
            }
            self.tick(Instant::now());
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.island.expire(now) {
            self.broadcast_present();
        }
        if self
            .handshake_deadline
            .is_some_and(|deadline| deadline <= now)
        {
            eprintln!("daemon: quickshell never said hello, restarting it");
            self.handshake_deadline = None;
            self.supervisor.restart();
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Connected { id, writer } => {
                self.clients.insert(id, Client { writer, role: None });
            }
            Event::Message { id, message } => self.handle_message(id, message),
            Event::Malformed { id, error } => {
                eprintln!("daemon: malformed message from connection {id}: {error}");
                self.reply(
                    id,
                    &Outgoing::Error {
                        message: format!("malformed message: {error}"),
                    },
                );
            }
            Event::Disconnected { id } => {
                if let Some(Client {
                    role: Some(Role::Ui),
                    ..
                }) = self.clients.remove(&id)
                {
                    eprintln!("daemon: ui disconnected (connection {id})");
                }
            }
            Event::UiStarted => self.handshake_deadline = Some(Instant::now() + HANDSHAKE_TIMEOUT),
        }
    }

    fn handle_message(&mut self, id: ConnectionId, message: Incoming) {
        let role = self.clients.get(&id).and_then(|client| client.role);

        match (role, message) {
            (None, Incoming::Hello { api, role }) => {
                if api != API {
                    self.reply(
                        id,
                        &Outgoing::Error {
                            message: format!("unsupported api {api}, expected {API}"),
                        },
                    );
                    self.drop_client(id);
                    return;
                }
                if let Some(client) = self.clients.get_mut(&id) {
                    client.role = Some(role);
                }
                self.reply(id, &Outgoing::Hello { api: API });

                if role == Role::Ui {
                    eprintln!("daemon: ui connected (connection {id})");
                    self.handshake_deadline = None;
                    let activity = self.island.current().clone();
                    self.reply(id, &Outgoing::Present { activity });
                }
            }
            (None, _) => {
                self.reply(
                    id,
                    &Outgoing::Error {
                        message: "say hello first".into(),
                    },
                );
                self.drop_client(id);
            }
            (Some(_), Incoming::Hello { .. }) => {
                self.reply(
                    id,
                    &Outgoing::Error {
                        message: "already said hello".into(),
                    },
                );
            }
            (Some(Role::Ui), Incoming::Event { activity, kind }) => {
                self.handle_ui_event(activity, kind)
            }
            (Some(_), Incoming::Event { .. }) => {
                self.reply(
                    id,
                    &Outgoing::Error {
                        message: "only the ui sends events".into(),
                    },
                );
            }
            (
                Some(_),
                Incoming::Command {
                    module,
                    action,
                    args,
                },
            ) => {
                let reply = match self.command(&module, &action, &args) {
                    Ok(()) => Outgoing::Ok,
                    Err(message) => Outgoing::Error { message },
                };
                self.reply(id, &reply);
            }
        }
    }

    fn handle_ui_event(&mut self, activity: u64, kind: EventKind) {
        // Events for an activity that is already gone arrive during morphs.
        if activity != self.island.current().id {
            return;
        }
        eprintln!("daemon: ui event {kind:?} on activity {activity}");
        let now = Instant::now();

        match kind {
            EventKind::HoverEnter => self.island.set_hovered(true, now),
            EventKind::HoverLeave => self.island.set_hovered(false, now),
            EventKind::Click => {
                if self.island.is_idle() && self.modules.iter().any(|module| module == "demo") {
                    self.island.present(
                        "demo",
                        "Card",
                        serde_json::json!({}),
                        Some(DEMO_TIMEOUT),
                        now,
                    );
                } else {
                    self.island.show_idle();
                }
                self.broadcast_present();
            }
        }
    }

    fn command(&mut self, module: &str, action: &str, args: &[String]) -> Result<(), String> {
        if !self.modules.iter().any(|enabled| enabled == module) {
            return Err(format!("module {module} is not enabled"));
        }

        match (module, action) {
            ("idle", "show") => self.island.show_idle(),
            ("demo", "show") => {
                let view = args
                    .first()
                    .ok_or("usage: demo show <Small|Wide|Card|Big> [text...]")?;
                if !assets::has_view("demo", view) {
                    return Err(format!("demo has no view {view}"));
                }
                let payload = serde_json::json!({ "text": args[1..].join(" ") });
                self.island
                    .present("demo", view, payload, Some(DEMO_TIMEOUT), Instant::now());
            }
            _ => return Err(format!("{module} has no action {action}")),
        }

        self.broadcast_present();
        Ok(())
    }

    fn broadcast_present(&mut self) {
        let activity = self.island.current().clone();
        eprintln!(
            "daemon: present {}/{} (activity {})",
            activity.module, activity.view, activity.id
        );
        let message = Outgoing::Present { activity };
        let ui: Vec<ConnectionId> = self
            .clients
            .iter()
            .filter(|(_, client)| client.role == Some(Role::Ui))
            .map(|(id, _)| *id)
            .collect();
        for id in ui {
            self.reply(id, &message);
        }
    }

    fn reply(&mut self, id: ConnectionId, message: &Outgoing) {
        let Some(client) = self.clients.get_mut(&id) else {
            return;
        };
        if let Err(error) = ipc::send(&mut client.writer, message) {
            eprintln!("daemon: dropping connection {id}: {error}");
            self.drop_client(id);
        }
    }

    fn drop_client(&mut self, id: ConnectionId) {
        if let Some(client) = self.clients.remove(&id) {
            let _ = client.writer.shutdown(std::net::Shutdown::Both);
        }
    }
}

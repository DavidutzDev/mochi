//! The Unix socket shared by Quickshell and the CLI.
//!
//! One thread accepts connections and one thread per connection reads lines.
//! Everything they receive goes to the daemon loop as an `Event`, so all state
//! lives on one thread.

use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use crate::Event;
use crate::protocol::{API, Incoming, Outgoing, Role};

pub type ConnectionId = u64;

/// A slow reader can't block the daemon for longer than this.
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

pub fn listen(path: &Path, events: Sender<Event>) -> io::Result<()> {
    if UnixStream::connect(path).is_ok() {
        return Err(io::Error::new(
            ErrorKind::AddrInUse,
            format!("another daemon is listening on {}", path.display()),
        ));
    }
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => return Err(error),
        _ => {}
    }

    let listener = UnixListener::bind(path)?;
    thread::Builder::new()
        .name("ipc-accept".into())
        .spawn(move || {
            for (id, stream) in (0..).zip(listener.incoming()) {
                match stream {
                    Ok(stream) => {
                        if accept(id, stream, &events).is_err() {
                            return;
                        }
                    }
                    Err(error) => eprintln!("ipc: accept failed: {error}"),
                }
            }
        })?;
    Ok(())
}

fn accept(id: ConnectionId, stream: UnixStream, events: &Sender<Event>) -> Result<(), ()> {
    let writer = match stream.try_clone() {
        Ok(writer) => writer,
        Err(error) => {
            eprintln!("ipc: could not clone connection: {error}");
            return Ok(());
        }
    };
    let _ = writer.set_write_timeout(Some(WRITE_TIMEOUT));

    // Sent before the reader starts, so the loop always sees Connected first.
    events.send(Event::Connected { id, writer }).map_err(drop)?;

    let events = events.clone();
    thread::spawn(move || {
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let event = match serde_json::from_str(&line) {
                Ok(message) => Event::Message { id, message },
                Err(error) => Event::Malformed {
                    id,
                    error: error.to_string(),
                },
            };
            if events.send(event).is_err() {
                return;
            }
        }
        let _ = events.send(Event::Disconnected { id });
    });
    Ok(())
}

pub fn send(stream: &mut UnixStream, message: &Outgoing) -> io::Result<()> {
    let mut line = serde_json::to_vec(message).map_err(io::Error::other)?;
    line.push(b'\n');
    stream.write_all(&line)
}

/// The client side of `mochi-spike ipc <module> <action> [args...]`.
pub fn call(path: &Path, module: &str, action: &str, args: &[String]) -> Result<(), String> {
    let mut stream = UnixStream::connect(path)
        .map_err(|error| format!("cannot reach the daemon at {}: {error}", path.display()))?;

    let messages = [
        Incoming::Hello {
            api: API,
            role: Role::Ctl,
        },
        Incoming::Command {
            module: module.to_owned(),
            action: action.to_owned(),
            args: args.to_vec(),
        },
    ];
    for message in &messages {
        let mut line = serde_json::to_vec(message).map_err(|error| error.to_string())?;
        line.push(b'\n');
        stream.write_all(&line).map_err(|error| error.to_string())?;
    }

    for line in BufReader::new(stream).lines() {
        let line = line.map_err(|error| error.to_string())?;
        match serde_json::from_str(&line).map_err(|error| error.to_string())? {
            Outgoing::Ok => return Ok(()),
            Outgoing::Error { message } => return Err(message),
            Outgoing::Hello { .. } | Outgoing::Present { .. } => {}
        }
    }
    Err("the daemon closed the connection without answering".into())
}

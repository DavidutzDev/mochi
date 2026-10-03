//! The Unix socket. Each connection gets a reader task and a writer task;
//! everything they receive goes to the daemon loop as a [`ConnectionEvent`],
//! so all state stays on the loop.

use std::io::{self, ErrorKind};
use std::path::Path;

use mochi_protocol::{ClientMessage, DaemonMessage, MAX_LINE};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::{self, UnboundedSender};

pub type ConnectionId = u64;

#[derive(Debug)]
pub enum ConnectionEvent {
    Opened {
        id: ConnectionId,
        sender: UnboundedSender<DaemonMessage>,
    },
    Message {
        id: ConnectionId,
        message: ClientMessage,
    },
    Malformed {
        id: ConnectionId,
        error: String,
    },
    Closed {
        id: ConnectionId,
    },
}

/// Binds the socket. Fails if another daemon answers on it, and replaces a
/// stale socket file left by one that crashed.
pub async fn bind(path: &Path) -> io::Result<UnixListener> {
    if UnixStream::connect(path).await.is_ok() {
        return Err(io::Error::new(
            ErrorKind::AddrInUse,
            format!("another mochid is running on {}", path.display()),
        ));
    }
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    UnixListener::bind(path)
}

pub fn accept_all(listener: UnixListener, events: UnboundedSender<ConnectionEvent>) {
    tokio::spawn(async move {
        for id in 0.. {
            match listener.accept().await {
                Ok((stream, _)) => {
                    if events.is_closed() {
                        return;
                    }
                    serve(id, stream, events.clone());
                }
                Err(error) => tracing::warn!(%error, "accept failed"),
            }
        }
    });
}

fn serve(id: ConnectionId, stream: UnixStream, events: UnboundedSender<ConnectionEvent>) {
    let (reader, mut writer) = stream.into_split();
    let (sender, mut outgoing) = mpsc::unbounded_channel::<DaemonMessage>();

    // Sent before the reader starts, so the loop sees Opened first.
    if events.send(ConnectionEvent::Opened { id, sender }).is_err() {
        return;
    }

    // The writer ends when the loop drops this connection's sender.
    tokio::spawn(async move {
        while let Some(message) = outgoing.recv().await {
            let line = match mochi_protocol::encode(&message) {
                Ok(line) => line,
                Err(error) => {
                    tracing::error!(%error, "could not encode a message");
                    continue;
                }
            };
            if writer.write_all(&line).await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });

    tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        loop {
            line.clear();
            let limit = MAX_LINE as u64 + 1;
            match (&mut reader).take(limit).read_until(b'\n', &mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() > MAX_LINE => {
                    let error = format!("line longer than {MAX_LINE} bytes");
                    let _ = events.send(ConnectionEvent::Malformed { id, error });
                    break;
                }
                Ok(_) => {}
            }

            let event = match std::str::from_utf8(&line)
                .map_err(|error| error.to_string())
                .and_then(|text| mochi_protocol::decode(text).map_err(|error| error.to_string()))
            {
                Ok(message) => ConnectionEvent::Message { id, message },
                Err(error) => ConnectionEvent::Malformed { id, error },
            };
            if events.send(event).is_err() {
                return;
            }
        }
        let _ = events.send(ConnectionEvent::Closed { id });
    });
}

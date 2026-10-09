//! Asking the running mochid, like `mochi` does: to reload after an
//! install, and to try a look.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use mochi_protocol::{API, ClientMessage, DaemonMessage, Role};

/// Why a request went nowhere: no daemon runs, or it refused.
#[derive(Debug)]
pub enum Failed {
    Unreachable,
    Refused(String),
}

pub fn request(message: &ClientMessage) -> Result<DaemonMessage, Failed> {
    let path = mochi_protocol::socket_path().ok_or(Failed::Unreachable)?;
    let mut stream = UnixStream::connect(&path).map_err(|_| Failed::Unreachable)?;
    let hello = ClientMessage::Hello {
        api: API,
        role: Role::Ctl,
    };
    for message in [&hello, message] {
        let line =
            mochi_protocol::encode(message).map_err(|error| Failed::Refused(error.to_string()))?;
        stream
            .write_all(&line)
            .map_err(|error| Failed::Refused(error.to_string()))?;
    }
    let mut lines = BufReader::new(stream).lines();
    loop {
        let line = lines
            .next()
            .ok_or_else(|| Failed::Refused("mochid closed the connection".into()))?
            .map_err(|error| Failed::Refused(error.to_string()))?;
        match mochi_protocol::decode(&line).map_err(|error| Failed::Refused(error.to_string()))? {
            DaemonMessage::Hello { .. } => {}
            DaemonMessage::Error { message, .. } => return Err(Failed::Refused(message)),
            answer => return Ok(answer),
        }
    }
}

/// Reloads the running mochid, if one runs, and says how it went.
pub fn reload() {
    match request(&ClientMessage::Reload) {
        Ok(_) => eprintln!("mochid reloaded"),
        Err(Failed::Unreachable) => {}
        Err(Failed::Refused(error)) => eprintln!("mochid didn't reload: {error}"),
    }
}

/// Runs a module action on the running mochid.
pub fn command(module: &str, action: &str, args: Vec<String>) -> Result<(), String> {
    match request(&ClientMessage::Command {
        module: module.to_owned(),
        action: action.to_owned(),
        args,
    }) {
        Ok(_) => Ok(()),
        Err(Failed::Unreachable) => Err("mochid isn't running".into()),
        Err(Failed::Refused(error)) => Err(error),
    }
}

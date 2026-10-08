//! The Mochi protocol: messages exchanged over the daemon's Unix socket.
//!
//! Every message is one JSON object on one line. Clients send a
//! [`ClientMessage`], the daemon sends a [`DaemonMessage`]. Each object has a
//! `type` field naming the variant in snake_case. `docs/protocol.md` describes
//! the conversation in full.
//!
//! The protocol only grows: new message types and new fields with defaults.
//! Removing or renaming anything means a new [`API`] version.

mod actions;
mod bubbles;
mod contributions;
mod messages;
pub mod plugin;
pub mod spec;
mod theme;

pub use actions::{ActionSpec, ArgKind, ArgSpec, ModuleActions};
pub use bubbles::{Area, Bubble, BubbleId, Overflow, Stacking};
pub use contributions::Contribution;
pub use messages::{
    Activity, ActivityId, ClientMessage, CompositorStatus, DaemonMessage, ErrorCode, EventKind,
    PluginState, PluginStatus, Role, Status,
};
pub use theme::{Anchor, Color, ColorError, Colors, Layout, Mode, Motion, Notch, Text, Theme};

/// The protocol version both sides announce in `hello`.
pub const API: u32 = 1;

/// The longest line a reader should accept, in bytes. Anything longer is a
/// broken or hostile peer.
pub const MAX_LINE: usize = 1 << 20;

/// Overrides where clients look for the daemon's socket. The daemon also sets
/// it for the Quickshell process it starts.
pub const SOCKET_ENV: &str = "MOCHI_SOCKET";

/// `$MOCHI_SOCKET`, or else `$XDG_RUNTIME_DIR/mochi/mochi.sock`.
pub fn socket_path() -> Option<std::path::PathBuf> {
    let set = |name| std::env::var_os(name).filter(|value| !value.is_empty());
    if let Some(socket) = set(SOCKET_ENV) {
        return Some(socket.into());
    }
    let runtime: std::path::PathBuf = set("XDG_RUNTIME_DIR")?.into();
    Some(runtime.join("mochi").join("mochi.sock"))
}

/// Serializes a message as one line, including the trailing newline.
pub fn encode<T: serde::Serialize>(message: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut line = serde_json::to_vec(message)?;
    line.push(b'\n');
    Ok(line)
}

/// Parses one line, with or without its trailing newline.
pub fn decode<'a, T: serde::Deserialize<'a>>(line: &'a str) -> Result<T, serde_json::Error> {
    serde_json::from_str(line.trim_end_matches(['\n', '\r']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_ends_with_one_newline() {
        let line = encode(&ClientMessage::Status).unwrap();
        assert_eq!(line, b"{\"type\":\"status\"}\n");
    }

    #[test]
    fn decode_accepts_the_trailing_newline() {
        let message: ClientMessage = decode("{\"type\":\"reload\"}\r\n").unwrap();
        assert_eq!(message, ClientMessage::Reload);
    }
}

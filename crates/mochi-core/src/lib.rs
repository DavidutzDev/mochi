//! The daemon's building blocks: the module API, the arbiter that decides
//! what the island shows, action argument parsing, configuration, the QML
//! asset writer and the Quickshell supervisor.
//!
//! The UI's core QML lives in this crate's `qml/` directory; see [`QML`].

pub mod actions;
pub mod arbiter;
pub mod assets;
pub mod bubbles;
pub mod changes;
pub mod config;
pub mod contributions;
pub mod examples;
pub mod islands;
pub mod module;
pub mod nix;
pub mod options;
pub mod palette;
pub mod process;
pub mod quality;
pub mod supervisor;
pub mod themes;

// Protocol types modules need, so a module only depends on this crate.
/// Compositor state and actions, from [`ModuleCtx::compositor`].
pub use mochi_compositor as compositor;
/// The variable naming the daemon's socket, for commands modules run.
pub use mochi_protocol::SOCKET_ENV;
/// Mochi's version, and the oldest one a package works with.
pub use mochi_protocol::version;
pub use mochi_protocol::{ActionSpec, ActivityId, Area, ArgSpec, BubbleId, Contribution};
/// For [`Module::settings_schema`].
pub use schemars;
/// For [`Module::check_settings`].
pub use toml;

pub use actions::{ArgError, ArgValue, Args};
pub use arbiter::{ActivitySpec, Arbiter, ArbiterError, Effect, EndReason, Priority, SamePriority};
pub use bubbles::{BubbleError, BubbleSpec, Bubbles, Placement};
pub use config::{
    BentoConfig, BubblesConfig, ClickOutside, Config, ConfigError, IslandConfig, Notices, Panels,
    Paths,
};
pub use contributions::ContributionSpec;
pub use islands::{Change, Islands};
pub use module::{
    ActivityIds, Assets, BoxFuture, CallError, Module, ModuleCommand, ModuleCtx, ModuleError,
    ModuleEvent, ModuleRequest, Need, Reply, Request, SettingsOp, settings,
};

/// The core QML: `shell.qml` and the island. Modules add their views under
/// `modules/<id>/` next to it.
pub const QML: Assets = {
    static DIR: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/qml");
    Assets::new(&DIR, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
};

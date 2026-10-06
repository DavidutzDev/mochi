//! The daemon's building blocks: the module API, the arbiter that decides
//! what the island shows, action argument parsing, configuration, the QML
//! asset writer and the Quickshell supervisor.
//!
//! The UI's core QML lives in this crate's `qml/` directory; see [`QML`].

pub mod actions;
pub mod arbiter;
pub mod assets;
pub mod bubbles;
pub mod config;
pub mod contributions;
pub mod examples;
pub mod module;
pub mod process;
pub mod quality;
pub mod supervisor;

// Protocol types modules need, so a module only depends on this crate.
/// Compositor state and actions, from [`ModuleCtx::compositor`].
pub use mochi_compositor as compositor;
pub use mochi_protocol::{ActionSpec, ActivityId, Area, ArgSpec, BubbleId, Contribution};
/// For [`Module::check_settings`].
pub use toml;

pub use actions::{ArgError, ArgValue, Args};
pub use arbiter::{ActivitySpec, Arbiter, ArbiterError, Effect, EndReason, Priority, SamePriority};
pub use bubbles::{BubbleError, BubbleSpec, Bubbles, Placement};
pub use config::{BubblesConfig, ClickOutside, Config, ConfigError, IslandConfig, Panels, Paths};
pub use contributions::ContributionSpec;
pub use module::{
    ActivityIds, Assets, BoxFuture, CallError, Module, ModuleCommand, ModuleCtx, ModuleError,
    ModuleEvent, ModuleRequest, Reply, Request, settings,
};

/// The core QML: `shell.qml` and the island. Modules add their views under
/// `modules/<id>/` next to it.
pub const QML: Assets = {
    static DIR: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/qml");
    Assets::new(&DIR, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
};

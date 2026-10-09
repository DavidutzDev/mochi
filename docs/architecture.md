# Architecture

For working on Mochi itself. The book's [How it works](book/src/how-it-works.md) explains the design to users; this page says where each part lives in the code and how a change travels through it.

## Crates

| Crate | What it holds |
|---|---|
| `crates/mochi-protocol` | The messages between `mochid`, the UI, the CLI and plugin backends, the theme's types, `API` and the version. Everything else depends on it. |
| `crates/mochi-core` | The `Module` trait and `ModuleCtx`, the arbiter that decides what each island shows, bubbles, `config.toml` and `theme.toml`, the settings panel's changes (`changes.rs`), the asset writer that builds the shell directory, the Quickshell supervisor, and the core QML in `qml/`. |
| `crates/mochi-compositor` | Workspaces, windows and outputs from standard Wayland protocols, with per-compositor extras behind one handle. |
| `crates/mochi-plugins` | Plugin manifests, `plugins.toml` and its lock file, installing plugins, and Bento's registry and packages. |
| `crates/mochid` | The daemon: the loop, the module runner, the socket, the settings store, plugins as modules, Bento's commands, the session lock and `mochid doctor`. |
| `crates/mochi` | The `mochi` CLI. |
| `crates/mochi-sdk` | The SDK for plugin backends written in Rust. |
| `modules/<id>` | One crate per builtin module: `src/`, `qml/`, and `settings.toml`, the commented example that also feeds the settings panel and the docs. |

## The daemon loop

`crates/mochid/src/daemon.rs` owns all state and is the only place it changes. `Daemon::run` selects over the socket's connections, the modules' requests and exits, the UI process's events, the compositor's outputs and the system's light or dark preference. After each event it ticks the islands' timers and sends what changed to the UI.

Modules run on tasks of their own (`crates/mochid/src/modules.rs`, `Runner`). They never touch the daemon's state: they send `ModuleRequest`s (present an activity, show a bubble, publish state, call another module) and receive `ModuleEvent`s (commands, clicks, settings). `ctx.call` reaches another module's action through the daemon, which checks it against the action's declared arguments first.

`Daemon::apply` makes the running modules match a config: it starts new ones, stops removed ones, restarts those whose settings changed (unless the module applies them live), writes the shell directory, and asks the UI to reload its views when a module is new to it.

## The UI

Quickshell runs `$XDG_RUNTIME_DIR/mochi/shell/shell.qml`, which `mochi_core::assets` writes from the core QML and each running module's `qml/`, copied or, in dev mode, linked to the source tree. `Supervisor` starts Quickshell, restarts it with a backoff when it exits, and gives up after repeated failures.

`qml/island/Daemon.qml` is the UI's one connection. On `hello` the daemon sends everything again: modules, contributions, each module's state, what each island shows, bubbles and the theme. Nothing in QML has to survive a restart or a reload. A module turned on sends `reload_views`, and the UI reloads in place with Quickshell's soft reload, which keeps its windows.

Views are `modules/<id>/<View>.qml`, loaded by URL with the module's state or the activity's payload as `payload`. Mochi's own views, like the hidden bubbles' list or the dev bubble, are in `qml/island/` under the module name `mochi`.

## Contributions

Modules offer each other views with `ContributionSpec::new(target, kind, id, view, title)`: cards and pages for `control-center`, providers for `launcher`, widgets for `widgets`, steps for `tour`, a `section` for `settings`. The UI reads them with `Daemon.offered(target, kind)`. A contribution whose view is missing is dropped with an error at start.

## Configuration and the settings panel

`config.toml` and `theme.toml` are read as tables. `crates/mochid/src/settings.rs`, the `Store`, lays the panel's changes from `changes.toml` over them, and what's being tried over those, then checks the result as the daemon would run it. A refused change never applies. The panel's pages come from each module's settings schema (`schemars`) and the comments of its `settings.toml`, so a setting needs no UI code.

Renamed module ids keep working through `mochi_protocol::RENAMED_MODULES`: the config is migrated when read, and commands and plugin manifests resolve the old id.

## Plugins

A plugin is a module from outside the binary: a manifest, QML views, and usually a backend that `mochid` starts and talks to over the socket with the same messages builtins use (`crates/mochid/src/plugins.rs`). `mochi plugins install` resolves sources into the lock file and builds them. Bento, in `crates/mochi-plugins/src/bento.rs` and `crates/mochid/src/bento/`, installs and shares themes, plugins and whole setups.

## One shell per session

`crates/mochid/src/session.rs` holds a lock per Wayland display in `$XDG_RUNTIME_DIR/mochi`, so a second daemon refuses to start. A dev daemon, the default of a debug build, sends the holder `step_aside`: it stops its modules and Quickshell, waits for the dev daemon's process to end, and starts again in the same process.

## Tests

- Unit tests next to the code, `cargo test --workspace`.
- End-to-end tests in `crates/mochid/tests` run the real daemon with a fake Quickshell and play the UI and the CLI over the socket.
- `views_check` compiles every QML view with Quickshell offscreen, so a typo in a view fails the build.
- `nix build .#mochi` runs all of them in a release build.

## Releases

A version tag runs `.github/workflows/release.yml`, which builds the archives, publishes the release from the changelog's section, and records it for the Nix and Arch packages. `RELEASING.md` has the steps. The universal installer, `install.sh`, installs and updates from those releases, and the updater module offers the update in the settings.

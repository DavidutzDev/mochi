# Mochi TODO

Mochi is a desktop shell built around a central island, like a dynamic island. A Rust daemon owns state and system integration. Quickshell renders the island.

This file lists the work grouped by area. The areas are not a build order. "Suggested order" at the end lists the order I'd start with.

## Decisions

- Two binaries, the same split as `Hyprland` and `hyprctl`:
  - `mochid` is the daemon and the only systemd unit. It supervises Quickshell as a child process.
  - `mochi` is the CLI. It depends only on `mochi-protocol`.
- The daemon decides and QML renders. State, policy, timeouts and system integration live in Rust. Layout and animation live in QML.
- The arbiter owns the island. Modules submit activities and the arbiter picks what shows.
- The island sizes itself to its content. The layer surface is a fixed transparent area with an input mask, and the island item animates to the content's implicit size.
- Each module crate embeds its `qml/` directory. At runtime `mochid` writes the enabled views to `$XDG_RUNTIME_DIR/mochi/shell/` and starts Quickshell on that directory.
- Builtin modules and plugins use the same interface. `ModuleCtx` maps one-to-one onto protocol messages, and an external plugin is an adapter that implements `Module` by forwarding to a process.
- Plugins are either QML-only (views and optional overrides) or have a backend, which is any executable that speaks the protocol. Every message passes through `mochid`. Plugins never talk to Quickshell directly.
- Plugins are trusted code. Their QML can already run commands, so there is no sandbox.
- The protocol is versioned from day one (`api = 1`). Changes only add fields.
- The CLI calls modules generically with `mochi ipc <module> <action> [args...]`.

## Repository layout

```
mochi-shell/
  Cargo.toml                 # workspace
  crates/
    mochi-protocol/          # message types, api version
    mochi-core/              # Module trait, ModuleCtx, arbiter, config,
      qml/                   #   asset writer, supervisor; core QML
    mochi-plugin/            # SDK for plugin backends
    mochid/                  # daemon binary
    mochi/                   # CLI binary
  modules/
    idle/  workspaces/  osd/  notifications/  launcher/  power/
                             # each: Cargo.toml, src/, qml/
  systemd/mochid.service
  docs/
```

## Runtime paths

| Path | Contents |
|---|---|
| `~/.config/mochi/config.toml` | enabled modules, per-module settings |
| `~/.config/mochi/theme.toml` | colors, radii, fonts, spring constants |
| `~/.local/share/mochi/plugins/<id>/` | user plugins (`mochi-plugin.toml`, `qml/`, optional `bin/`) |
| `$XDG_RUNTIME_DIR/mochi/mochi.sock` | IPC socket for Quickshell and `mochi` |
| `$XDG_RUNTIME_DIR/mochi/shell/` | generated shell tree that Quickshell loads |

## Spike (throwaway)

Answers the risky questions before the framework depends on them. Done: code in `spike/`, results in `docs/spike.md`.

- [x] Layer surface with a large transparent area and an input mask that follows the island item
- [x] Island resizes with springs to whatever content is loaded into it
- [x] Clicks outside the mask reach the windows underneath
- [x] Exclusive zone stays at the idle height while the island grows
- [x] Blur applies to the island only, using Hyprland layer rules (`ignore_alpha`)
- [x] Morph: load the next view invisibly, read its implicit size, animate the container, then crossfade
- [x] A small Rust program spawns `quickshell -p <tmpfs dir>`, sends JSON lines over a socket and receives clicks back
- [x] Hot reload works when the shell tree is made of symlinks
- [x] Write down the findings in `docs/spike.md` and pin the Quickshell version
- [x] Delete `spike/` (kept in commit `c441f31`)

## Protocol (`mochi-protocol`)

Described in `docs/protocol.md`.

- [x] Newline-delimited JSON over a Unix socket, 1 MiB line limit
- [x] `hello {api}` handshake in both directions, `unsupported_api` error on mismatch
- [x] Daemon to UI: `modules`, `state`, `present`, `theme` (no `dismiss`: `present` always names what is shown, `null` when nothing is)
- [x] UI to daemon: `command`, `event` (click, hover enter and leave, dismiss)
- [x] CLI to daemon: `command`, `status`, `reload`, `list_actions`
- [x] Error replies with a code and a message
- [x] Round-trip tests for every message type, plus fixed wire-format tests for what the UI parses
- [x] `docs/protocol.md`
- [ ] Plugin management messages (with the plugin system)
- [x] State snapshots re-sent on every reconnect (daemon, step 6)

## Core (`mochi-core`)

Module API
- [x] `Module` trait: `id`, `assets`, `actions`, `run(ctx)`. Each module runs as its own task and receives commands and activity events through its context, instead of `start` plus `handle_command` (an async `&mut self` method would make `Module` unusable as a trait object)
- [x] `ModuleCtx`: `publish_state`, `present`, `update`, `withdraw`, typed `settings`, `next_event`
- [x] Action declarations with argument names and types (`string`, `int`, `float`, `bool`, `choice`, optional and rest arguments)
- [x] Argument parsing and spec validation, with usage lines in errors
- [x] Route `command` messages to modules (daemon, step 6)

Arbiter
- [x] Activity attributes: `key`, `compact` and `expanded` views, `payload`, `priority`, `timeout`, `interruptible`, `same_priority` (queue or stack)
- [x] Interrupt and resume, queue by priority then arrival, suspended activities win ties
- [x] Replacement by `key` within a module
- [x] Timeouts pause while hovered, expanded or interrupted, and resume with at least 1 second
- [x] Click toggles compact and expanded; clicks on activities without an expanded view go to the module
- [x] Unit tests for every rule above
- [ ] Per-module or per-action overrides of activity attributes in `config.toml`, once real modules show which rules users want to change
- [ ] `split` state (two bubbles side by side), if it still looks worth it

Config and theme
- [x] Load and validate `config.toml` and `theme.toml`, with errors that name the file and the key
- [x] Theme tokens: colors, layout sizes, spring and fade constants
- [x] XDG paths for config, socket and the generated shell
- [ ] Fonts in the theme
- [x] `mochi reload` reloads `theme.toml` without restarting Quickshell
- [ ] Apply `config.toml` changes on reload (enabling and disabling modules at runtime)

Asset writer
- [x] Write the core QML and each enabled module's QML into `$XDG_RUNTIME_DIR/mochi/shell/`
- [x] Rewrite a file only when its content hash changed, so restarts don't force a UI reload
- [ ] Add and remove module directories at runtime when modules are enabled or disabled
- [x] Dev mode: symlink to the source `qml/` directories instead of copying
- [x] Generate `Modules.qml` at the shell root importing every enabled module, so Quickshell watches module files (see `docs/spike.md`)
- [x] Write through a temporary file and a rename, so Quickshell never reads a half-written file

Supervisor
- [x] Spawn Quickshell with `-p <shell dir>` and `MOCHI_SOCKET` set
- [x] `PR_SET_PDEATHSIG` so Quickshell dies with `mochid`, spawned from a dedicated thread that lives as long as the daemon (the signal follows the thread, not the process)
- [x] Pipe stdout and stderr into `tracing`
- [x] Restart with backoff, give up after 5 crashes in 30 seconds and log why
- [x] Restart if `hello` doesn't arrive within the handshake timeout, with SIGKILL (a hung process ignores SIGTERM)
- [x] Check the Quickshell version at startup
- [ ] The same supervisor runs plugin backends

## Daemon (`mochid`)

- [x] Entry point with flags: `--dev`, `--config`, `--modules`, `--runtime-dir`, `--quickshell`
- [x] Logging through `tracing`, readable in `journalctl --user -u mochid`
- [x] Socket server that accepts Quickshell and `mochi` connections (plugins later)
- [x] Claim the socket before writing assets, and refuse to start when another daemon is running
- [x] Clean shutdown on SIGTERM: dismiss everything, stop Quickshell and plugins
- [x] `systemd/mochid.service` with `Restart=on-failure`, bound to `graphical-session.target`, `ExecReload` running `mochi reload`

## CLI (`mochi`)

- [x] `mochi ipc <module> <action> [args...]`, forwarded as `command`
- [x] `mochi ipc list` lists every module and its actions
- [x] `mochi ipc <module>` lists one module's actions with argument help
- [x] `mochi status` shows daemon and UI health (plugins later)
- [x] `mochi reload` reloads the theme (config changes still need a restart)
- [ ] `mochi plugins list|enable|disable`
- [x] Clear error when `mochid` isn't running
- [ ] `--json` output for scripts
- [ ] Shell completions

## QML core

- [x] Socket client that reads `MOCHI_SOCKET` and reconnects
- [x] Island container: springs on width, height and radius, morph between views, input mask
- [x] Two loaders taking turns, loading views through `root:/modules/<id>/<View>.qml` URLs (plain file paths break singletons and hot reload)
- [ ] Decide whether views are revealed from the center or anchored to the top edge during a morph
- [ ] Fall back to the builtin view when a view fails to load
- [x] Theme singleton fed by the `theme` message
- [x] Forward hover, click and dismiss events to the daemon
- [x] Re-send `hover_enter` for the new activity when the island changes while the pointer is still over it (the arbiter ignores events for activities that are no longer shown)

## Compositor support

Mochi must work on any wlroots-style compositor (Hyprland, niri, mango, ...). Standard Wayland protocols come first, and compositor-specific code lives behind one adapter in the daemon.

- [x] Blur through `ext-background-effect-v1`, no compositor rule needed (tested on Hyprland)
- [ ] Check which protocols niri, mango and Sway support: `ext-background-effect-v1`, `ext-workspace-v1`, `wlr-layer-shell`
- [ ] `Compositor` trait in the daemon: detection from the environment (`HYPRLAND_INSTANCE_SIGNATURE`, `NIRI_SOCKET`, ...), workspaces, focused output
- [ ] Generic adapter on `ext-workspace-v1` for compositors that support it
- [ ] Hyprland adapter (socket2 events, `hyprctl`)
- [ ] niri adapter (`NIRI_SOCKET` JSON IPC)
- [ ] mango adapter
- [ ] Fallback for compositors without background effects: apply a runtime rule where possible (Hyprland `hyprctl eval`), otherwise `mochi setup <compositor>` prints the config snippet
- [ ] `mochi status` shows the detected compositor and which features are active

## Modules

### Idle

- [x] Lowest-priority activity that never times out
- [x] Clock or a simple pill as the first view
- [x] A `demo` module (Cargo feature, on by default) with test views and `show`, `alert`, `stack`, `volume` and `clear` actions for trying the arbiter

### Workspaces

- [ ] Workspace events from the compositor adapter, never from a compositor directly
- [ ] Publish workspace state
- [ ] Short compact activity on workspace switch
- [ ] Multi-monitor behavior (see open questions)

### OSD

- [ ] Volume through PipeWire or wireplumber
- [ ] Brightness through logind `SetBrightness`
- [ ] Actions: `mochi ipc osd volume +5`, `mochi ipc osd brightness -10`
- [ ] Interrupts the current activity and lets it resume afterwards

### Notifications

- [ ] Own `org.freedesktop.Notifications` with zbus
- [ ] Actions, replaces-id, urgency, expiry, icons and images
- [ ] History with persistence across daemon restarts
- [ ] Do-not-disturb (`mochi ipc notifications dnd on|off|toggle`)
- [ ] Grouping per app
- [ ] Compact, expanded and history views

### Launcher

- [ ] Index desktop entries and watch the directories for changes
- [ ] Fuzzy search with `nucleo` in the daemon, results streamed to the UI
- [ ] Keyboard focus on the layer surface
- [ ] `mochi ipc launcher toggle`, bound in Hyprland
- [ ] Launch through `systemd-run --user --scope` or `uwsm app` so apps aren't children of `mochid`

### Power

- [ ] Shutdown, reboot and suspend through logind
- [ ] Lock: choose between Quickshell `WlSessionLock` and handing off to hyprlock
- [ ] Confirmation step in the expanded view
- [ ] Actions: `mochi ipc power shutdown|reboot|suspend|lock`

## Plugins

- [ ] `mochi-plugin.toml`: `id`, `api`, `overrides`, `backend`, `actions`
- [ ] Scan `~/.local/share/mochi/plugins/` and report invalid manifests clearly
- [ ] Reject plugins with an unsupported `api` version
- [ ] QML-only plugins: add their views to the asset writer
- [ ] View overrides (`overrides = ["idle/Compact"]`) with fallback to the builtin view
- [ ] Backend plugins: spawn with a `socketpair`, supervise them, wrap them in the `Module` adapter
- [ ] Plugin actions reachable through `mochi ipc <plugin> <action>`
- [ ] Enable and disable plugins at runtime
- [ ] `mochi-plugin` SDK crate
- [ ] One example QML-only plugin and one example backend plugin
- [ ] `docs/plugins.md`

## Integration and docs

- [x] Session setup notes: uwsm and `graphical-session.target`, or starting `mochid` from the compositor's autostart (README)
- [ ] Keybind examples for `mochi ipc` in Hyprland's Lua config
- [ ] Decide how the island and Waybar share the top edge, or whether Mochi replaces Waybar
- [x] Install instructions (Nix package and cargo)
- [x] Flake `packages` output: both binaries, the systemd unit, and `mochid` wrapped with the pinned Quickshell
- [x] `nix flake check` builds the package and runs the test suite
- [x] End-to-end daemon tests with a fake Quickshell (run in the Nix build)
- [x] Recording test against the real binaries (`cargo test -p mochid --test record -- --ignored`), writing outside the repository
- [ ] NixOS or home-manager module
- [ ] `docs/architecture.md`

## Later

- [ ] Lua or WASM plugin backends
- [ ] SDKs for other languages
- [ ] Settings UI

## Open questions

1. Multiple monitors: one island per output, or only on the focused output?
2. Can a plugin read state from other modules? Proposal: yes, through dependencies declared in the manifest.
3. Is the lock screen a module, or a separate minimal program? A crash in the lock screen is a security problem.
4. Does the theme control the animation springs, or are they fixed per view?

## Suggested order

Phase 1 (the framework, the idle module and packaging) is done.


1. Spike
2. Protocol, core, daemon, CLI and QML core, with the idle module as the first user
3. Workspaces
4. OSD
5. Notifications
6. Launcher
7. Power
8. Plugins

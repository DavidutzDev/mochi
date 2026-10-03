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

Answers the risky questions before the framework depends on them.

- [ ] Layer surface with a large transparent area and an input mask that follows the island item
- [ ] Island resizes with springs to whatever content is loaded into it
- [ ] Clicks outside the mask reach the windows underneath
- [ ] Exclusive zone stays at the idle height while the island grows
- [ ] Blur applies to the island only, using Hyprland layer rules (`ignorealpha`)
- [ ] Morph: load the next view invisibly, read its implicit size, animate the container, then crossfade
- [ ] A small Rust program spawns `quickshell -p <tmpfs dir>`, sends JSON lines over a socket and receives clicks back
- [ ] Hot reload works when the shell tree is made of symlinks
- [ ] Write down the findings in `docs/spike.md` and pin the Quickshell version

## Protocol (`mochi-protocol`)

- [ ] Newline-delimited JSON over a Unix socket
- [ ] `hello {api}` handshake in both directions, reject unsupported versions with a clear error
- [ ] Daemon to UI: `modules`, `state`, `present`, `dismiss`, `theme`
- [ ] UI to daemon: `command`, `event` (hover, click, dismissed)
- [ ] CLI to daemon: `command`, `status`, `reload`, `list_actions`, plugin management
- [ ] Error replies with a code and a message
- [ ] State snapshots re-sent on every reconnect
- [ ] Round-trip tests for every message type
- [ ] `docs/protocol.md`

## Core (`mochi-core`)

Module API
- [ ] `Module` trait: `id`, `assets`, `actions`, `start`, `handle_command`
- [ ] `ModuleCtx`: `publish_state`, `submit_activity`, `dismiss`, settings access
- [ ] Action declarations with argument names and types
- [ ] Route `command` messages to modules and validate actions and arguments before the module sees them

Arbiter
- [ ] Activity type: `{module, view, data, priority, timeout, interruptible}`
- [ ] Priority queue with `idle`, `compact` and `expanded` states
- [ ] Timeouts, with hover pausing the timeout
- [ ] Interrupting an activity and resuming it afterwards
- [ ] `split` state (two bubbles side by side), if it still looks worth it after the spike
- [ ] Unit tests for every rule above

Config and theme
- [ ] Load and validate `config.toml` and `theme.toml`, with errors that name the file and the key
- [ ] Theme tokens: colors, radii, fonts, spring constants
- [ ] Reload both files on `mochi reload` without restarting Quickshell

Asset writer
- [ ] Write the core QML and each enabled module's QML into `$XDG_RUNTIME_DIR/mochi/shell/`
- [ ] Rewrite a file only when its content hash changed, so restarts don't force a UI reload
- [ ] Add and remove module directories at runtime when modules are enabled or disabled
- [ ] Dev mode: symlink to the source `qml/` directories instead of copying

Supervisor
- [ ] Spawn Quickshell with `-p <shell dir>` and `MOCHI_SOCKET` set
- [ ] `PR_SET_PDEATHSIG` so Quickshell dies with `mochid`
- [ ] Pipe stdout and stderr into `tracing`
- [ ] Restart with backoff, give up after 5 crashes in 30 seconds and log why
- [ ] Restart if `hello` doesn't arrive within the handshake timeout
- [ ] Check the Quickshell version at startup
- [ ] The same supervisor runs plugin backends

## Daemon (`mochid`)

- [ ] Entry point with flags: `--dev`, `--config`
- [ ] Logging through `tracing`, readable in `journalctl --user -u mochid`
- [ ] Socket server that accepts Quickshell, `mochi` and plugin connections
- [ ] Clean shutdown on SIGTERM: dismiss everything, stop Quickshell and plugins
- [ ] `systemd/mochid.service` with `Restart=on-failure`, bound to `graphical-session.target`

## CLI (`mochi`)

- [ ] `mochi ipc <module> <action> [args...]`, forwarded as `command`
- [ ] `mochi ipc list` lists every module and its actions
- [ ] `mochi ipc <module>` lists one module's actions with argument help
- [ ] `mochi status` shows daemon, Quickshell and plugin health
- [ ] `mochi reload` reloads config and theme
- [ ] `mochi plugins list|enable|disable`
- [ ] Clear error when `mochid` isn't running
- [ ] `--json` output for scripts
- [ ] Shell completions

## QML core

- [ ] Socket client that reads `MOCHI_SOCKET` and reconnects
- [ ] Island container: springs on width, height and radius, morph between views, input mask
- [ ] One `Loader` per activity, loading `modules/<id>/<View>.qml`
- [ ] Fall back to the builtin view when a view fails to load
- [ ] Theme singleton fed by the `theme` message
- [ ] Forward hover, click and dismiss events to the daemon

## Modules

### Idle

- [ ] Lowest-priority activity that never times out
- [ ] Clock or a simple pill as the first view

### Workspaces

- [ ] Hyprland socket2 listener with reconnect
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

- [ ] Hyprland setup notes: `exec-once = systemctl --user start mochid`, environment import, keybind examples
- [ ] Layer rules for blur
- [ ] Install instructions (cargo, and later a Nix package)
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

1. Spike
2. Protocol, core, daemon, CLI and QML core, with the idle module as the first user
3. Workspaces
4. OSD
5. Notifications
6. Launcher
7. Power
8. Plugins

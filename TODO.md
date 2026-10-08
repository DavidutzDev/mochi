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
    mochi-plugins/           # manifests, plugins.toml, installing
    mochi-sdk/               # SDK for plugin backends
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
| `~/.config/mochi/plugins.toml`, `plugins.lock` | the plugins to install, and what each resolved to |
| `~/.local/share/mochi/plugins/<id>/` | installed plugins (`mochi-plugin.toml`, `qml/`, the backend) |
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
- [x] Plugin management: `status` lists plugins with their state; installing stays in the CLI, which needs no daemon
- [x] State snapshots re-sent on every reconnect (daemon, step 6)

## Core (`mochi-core`)

Module API
- [x] `Module` trait: `id`, `assets`, `actions`, `run(ctx)`. Each module runs as its own task and receives commands and activity events through its context, instead of `start` plus `handle_command` (an async `&mut self` method would make `Module` unusable as a trait object)
- [x] `ModuleCtx`: `publish_state`, `present`, `update`, `withdraw`, typed `settings`, `next_event`
- [x] `ModuleCtx::data_dir`: a per-module directory under the runtime directory, emptied at startup, for files views load
- [x] Contributions: `Module::contributions()` declares what a module offers another (`target`, `kind`, `id`, `view`, `title`, `icon`, `order`, `options`); the daemon checks the views exist and sends them all to the UI in a `contributions` message. Unused when the target isn't enabled, which makes them soft dependencies
- [x] `ModuleCtx::call(module, action, args)`: runs another module's action with the same checks as `mochi ipc`; `CallError::NotEnabled` when it isn't there, `Itself` for a module calling itself. The future doesn't borrow the context, so it can be spawned instead of awaited where two modules might call each other
- [x] Watching another module's published state: `ModuleCtx::watch_state` and `ModuleEvent::State`
- [x] Action declarations with argument names and types (`string`, `int`, `float`, `bool`, `choice`, optional and rest arguments)
- [x] Argument parsing and spec validation, with usage lines in errors
- [x] Route `command` messages to modules (daemon, step 6)

Arbiter
- [x] Activity attributes: `key`, `compact` and `expanded` views, `payload`, `priority`, `timeout`, `interruptible`, `same_priority` (queue or stack)
- [x] Interrupt and resume, queue by priority then arrival, suspended activities win ties
- [x] Replacement by `key` within a module
- [x] Timeouts pause while hovered, expanded or interrupted, and resume with at least 1 second
- [x] Click toggles compact and expanded; clicks on activities without an expanded view go to the module
- [x] `expand_for`: an activity opens on its expanded view and collapses on its own after that time on screen; hovering pauses it, a click hands control to the user, a keyed replacement that sets it again opens it again, and a waiting activity expands when it comes on screen
- [x] Unit tests for every rule above
- [ ] Per-module or per-action overrides of activity attributes in `config.toml`, once real modules show which rules users want to change
- [ ] `split` state (two bubbles side by side), if it still looks worth it

Config and theme
- [x] Load and validate `config.toml` and `theme.toml`, with errors that name the file and the key
- [x] Theme tokens: colors, layout sizes, spring and fade constants
- [x] XDG paths for config, socket and the generated shell
- [x] Fonts in the theme: `family`, and `display_family` for the clocks
- [x] `mochi reload` reloads `theme.toml` without restarting Quickshell
- [x] Apply `config.toml` changes on reload: new modules start, removed ones stop, changed ones restart, the rest keep running; a broken file changes nothing. Each start has a generation, so a replaced module's exit is ignored
- [x] Generated examples: the first start writes commented `config.toml` and `theme.toml`. Each module's section is its own `settings.toml` (`Module::settings_example`), the theme and bubbles are in `crates/mochi-core/defaults`; tests uncomment the `# key = value` lines and check they parse to the real defaults
- [x] `Module::check_settings`: every module's settings are checked at startup, on reload and by `mochi config check`, with the file, section and key in the error
- [x] `mochi config init [--print] | check | path`, through `mochid config` so the CLI stays free of module code

Asset writer
- [x] Write the core QML and each enabled module's QML into `$XDG_RUNTIME_DIR/mochi/shell/`
- [x] Rewrite a file only when its content hash changed, so restarts don't force a UI reload
- [x] Add and remove module directories at runtime when modules are enabled or disabled
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
- [x] Plugin backends are supervised the same way: backoff, then giving up after 5 crashes in a minute

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
- [x] `mochi reload` reloads the theme and the config
- [x] `mochi plugins list|install|update|remove`; enabling stays in `config.toml`'s `modules`
- [x] Clear error when `mochid` isn't running
- [x] `--json` output for scripts: `status`, `ipc list`, `ipc <module>`, `plugins list`
- [x] Shell completions: `mochi completions <shell>`, installed by the Nix and Arch packages

## QML core

- [x] Socket client that reads `MOCHI_SOCKET` and reconnects
- [x] Island container: springs on width, height and radius, morph between views, input mask
- [x] Two loaders taking turns, loading views through `root:/modules/<id>/<View>.qml` URLs (plain file paths break singletons and hot reload)
- [ ] Decide whether views are revealed from the center or anchored to the top edge during a morph
- [x] Fall back to the builtin view when a view fails to load
- [x] Theme singleton fed by the `theme` message
- [x] Forward hover, click and dismiss events to the daemon
- [x] Re-send `hover_enter` for the new activity when the island changes while the pointer is still over it (the arbiter ignores events for activities that are no longer shown)
- [x] A click outside closes whatever the island shows, except the idle island and passive notices (the volume and workspace OSDs, `ActivitySpec::passive`): one the user opened, or a modal one, is dismissed; others end with `EndReason::Outside`, so a notification goes to the missed ones. The island catches every click while such a view shows, without the keyboard. `[island] click_outside = "expanded"` limits it to views the user opened

## Compositor support

Mochi must work on any compositor that speaks the standard protocols (Hyprland, niri, mango, ...). The daemon is a Wayland client of its own and uses standard protocols first; compositor IPC only fills gaps the standards leave. Modules only ever see `mochi-compositor`'s state and actions, never a compositor directly.

- [x] Blur through `ext-background-effect-v1`, no compositor rule needed (tested on Hyprland)
- [x] `mochi-compositor` crate: one `Compositor` handle for modules, with a `State` snapshot (outputs, workspaces), change notifications and actions
- [x] Wayland backend: `ext-workspace-v1` for workspaces (per output, active, urgent, hidden, coordinates) and `wl_output` v4 for output names, with output hotplug
- [x] Action: switch to a workspace
- [x] Focused output: from the focused window (`wlr-foreign-toplevel-management`, standard), and on Hyprland exactly from its event socket, which also covers empty workspaces; modules only see `State::focused_output`
- [x] Without a supported compositor, an `unsupported` state instead of an error; modules keep running
- [x] `ModuleCtx::compositor()` for modules; `mochi status` shows the backend, outputs and workspace count
- [x] Pure model unit-tested; live test against the session's compositor (`cargo test -p mochi-compositor --test live -- --ignored`), passing on Hyprland 0.56
- [x] Check which protocols niri and Sway support: niri 25.08 and Sway 1.12 have `ext-workspace-v1`, both have `wlr-foreign-toplevel-management`, niri 26.04 has `ext-background-effect-v1`; `mochi doctor` lists them for any compositor
- [ ] The same for mango
- [ ] Windows (title, app id) in the state, from the toplevel protocol already bound for focus, when a module needs them
- [x] niri and Sway: the focused output, windows and (niri) screencasts from their IPC
- [ ] Keyboard layout, which has no standard protocol: per-compositor IPC (Hyprland first) behind the same handle
- [ ] Fallback for compositors without background effects: apply a runtime rule where possible (Hyprland `hyprctl eval`), otherwise `mochi setup <compositor>` prints the config snippet

## Design system

"Obsidian", picked on 2026-10-03 after looking at end-4's illogical-impulse and caelestia: layered surfaces, a fixed radius scale, a type scale, motion with a slight overshoot, and the accent only on what matters.

- [x] Color roles in `theme.toml`: `background` (a black island, 96%), `surface` for cards, `raised` for controls, tracks and dividers, `highlight` for hovered controls, `foreground`, `muted`, `accent` with `on_accent`, `danger`, `success`
- [x] Radius scale `radius_small` 8, `radius_medium` 14, `radius_large` 20, and `max_radius` 34 so the hub's cards sit concentric in its 14px padding
- [x] Type scale in `[text]`: caption 11, label 12, body 13, subtitle 14, title 16, headline 20, display 42, and `family` (empty keeps the system font); every view uses it
- [x] Motion: `fast_ms` for colors and hovers, `move_ms` with Material 3's expressive curve (`0.38, 1.21, 0.22, 1`) for things that move, on the navbar, the power profiles and the workspace pill
- [x] Every view moved to roles: no raw colors or sizes outside the demo module
- [ ] Colors generated from the wallpaper, as an option of the same roles (see Polish and features)
- [ ] Light variant of the palette (see Polish and features)
- [x] Built-in controls in the core: `Button`, `IconButton`, `Slider`, `ProgressBar`, `Switch`, `Segmented`, `Tile`, `ListRow`, `Badge`, `SectionLabel`, all drawn from theme roles. Media, notifications, the OSD, power and the launcher use them; their private buttons and icon sets are gone, and every icon lives in `Symbol`. `mochi ipc demo controls` shows them all; `docs/views.md` describes them

## Layout

Where the island sits and what shape it takes. Island mode floats it as a pill, `margin` away from the edge. Notch mode attaches it to the edge, like a MacBook notch: square where it meets the edge, with concave rounded corners ("ears") that flare into the edge, and round corners on the free side. Built on 2026-10-03, before bubbles, since the bubble shapes depend on it.

Settings in `theme.toml`, applied by `mochi reload`:

```toml
[layout]
mode = "island"     # or "notch"
anchor = "top"      # or "bottom"
island = "center"   # the area the island sits in, see Bubbles
margin = 6          # gap to the edges in island mode; notch mode is always 0
spacing = 8         # gap between the island and pills, and between areas

[layout.notch]
ear_radius = 10
```

- [x] Theme: `mode`, `anchor`, `island`, `margin`, `spacing` and `[layout.notch] ear_radius`; `top_margin` still reads as `margin`
- [x] Layer surface on the top or bottom edge; the island grows away from the edge it's attached to
- [x] `IslandShape`, one SVG path instead of the rounded `Rectangle`: worked out for the top edge and the left side, mirrored for the other anchors. A pill in island mode; in notch mode square on the attached edges, ears where it meets them, rounded free corners
- [x] In notch mode, the outermost shape of the left and right areas touches both edges, with an ear along each and only the corner facing the screen round. The corner anchors and `offset` from the first version became `island = "left"` or `"right"` once the five areas existed
- [x] Input mask and blur region follow the shape: per-corner radii, and for the blur each ear is a square with a circle subtracted (child regions use window coordinates, checked with a test shell)
- [x] Exclusive zone on the attached edge: the idle island plus the margin, for every anchor, so the idle island never covers windows
- [x] Switching mode on reload morphs: the island slides to the edge while its corners square off and the ears grow
- [x] Views don't change, and the idle module still decides what idle shows
- [x] Tested on Hyprland: top, bottom, both bottom corners, island and notch, the morph, expanding and collapsing by click
- [ ] Switching the anchor jumps instead of moving, since the layer surface changes edge
- [ ] Under another layer surface with an exclusive zone, like a bar, the notch attaches to that surface's edge, not the screen's. Matching the bar's color makes them look like one piece; a `[layout.notch] color` could help
- [ ] Per-output layout once per-output islands exist
- [x] `[island] panels = "focus" | "pointer" | "all"`: the daemon gives a panel (a modal activity without an overlay) an `output`, the focused monitor or the one under the pointer (Hyprland's `cursorpos`), and the islands on other monitors keep what they showed. Each island reports events for the activity it shows

## Bubbles

Small, long-lived status items owned by modules: music while it plays, earbuds battery while they're connected, a microphone-in-use indicator, a running timer. Activities are short-lived and one at a time; bubbles last as long as their condition holds and several show at once. Built on 2026-10-03.

The edge has five areas, from left to right: `left`, `center-left`, `center`, `center-right` and `right`. The side areas sit against the screen's sides with the margin; the center ones hug whatever is in the center, or meet in the middle when it's empty. The island sits in one area (`[layout] island`, `center` by default): at the screen edge in `left` and `right`, next to the center in `center-left` and `center-right`.

```toml
# config.toml
[bubbles]
max_per_area = 4

[bubbles.media]     # any module id: overrides what the module chose
area = "left"
group = "status"    # bubbles with the same group in an area share a pill; "" for its own
order = 1           # lower goes further left
wide = true         # the module's wide views with text, in pills
```

- [x] `ModuleCtx`: `show_bubble(spec)`, `update_bubble(id, payload)`, `hide_bubble(id)`; `BubbleSpec` with `key`, view, payload, area, group, order and priority. The module picks all of them
- [x] Small by default: a bubble's view fits about 26 pixels and sits in a circle, and a group of them shares a capsule. A module can add a wide view with text, which the user turns on with `wide = true` and which shows in a pill. Media's small view is the cover with a progress ring; its wide one has the title and bars
- [x] Clicks reach the module as `ModuleEvent::BubbleClicked`; media answers by putting the player back on the island
- [x] `Bubbles` board next to the arbiter, pure and unit-tested: the user's `[bubbles.<module>]` placement wins over the module's, areas sort by order, then priority, then age, and a group takes the place of its first member with the rest following it
- [x] Keyed bubbles replace in place and keep their spot; the UI keeps their view and updates the payload
- [x] Overflow: past `max_per_area`, an area leaves out its lowest priorities and shows a "+N" pill
- [x] Protocol: a `bubbles` snapshot after `hello` and on every change, `bubble_click` from the UI (both additions, API 1)
- [x] UI: five rows in the island's layer surface. Pills have the island's shape, so notch mode attaches them to the edge with ears, and the outermost pill of a side area curves into the screen corner. The island's slot takes its animated size, so pills slide outward as it grows. New pills grow in
- [x] Input mask and blur region cover the island and every pill
- [x] Demo actions `bubble <name> <area> [group]` and `pop <name>`; clicking a demo bubble names it on the island
- [x] Tested on Hyprland: every area, a group, a click through the input mask, the island pushing pills, notch mode, the island in the left area, overflow with a maximum of 2
- [x] Clicking "+N" lists the hidden bubbles in the island
- [x] Pills shrink and fade out, a bubble leaving a group fades while the pill narrows, and the others slide instead of jumping when one comes or goes
- [ ] Notch mode: fuse adjacent pills and the island into one outline instead of separate tabs whose ears overlap
- [ ] Placement per bubble key, not only per module, for modules with several bubbles
- [x] Stacked bubbles, an option: one per area, the most important in front, news brought forward for a while, fanned out on hover
- [ ] Parked: switching a recording's source while it records. A fixed 1080p frame drawn by Mochi on a virtual monitor, regions zoomed to fill it with animated pans; needs a spike first, as gpu-screen-recorder likely can't capture a headless monitor except through the portal
- [x] Bubbles off per module: `show = false` in `[bubbles.<module>]`
- [x] `mochi reload` applying `[bubbles]` changes
- [x] Plugin backends get the same calls through the protocol
- [x] First real user beyond media: a Bluetooth module (connected device battery from BlueZ over D-Bus)

## Modules

### Idle

- [x] Lowest-priority activity that never times out
- [x] Clock or a simple pill as the first view
- [x] A click runs another module's action through `ctx.call`: `click = ["hub", "toggle"]` by default, nothing when that module isn't enabled
- [x] A `demo` module (Cargo feature, on by default) with test views and `show`, `alert`, `stack`, `volume`, `bubble`, `pop` and `clear` actions for trying the arbiter and bubbles

### Workspaces

- [x] Workspace changes from `ModuleCtx::compositor()`, never from a compositor directly
- [x] A pure tracker compares snapshots by output and workspace name: a switch wins over an urgent workspace, which wins over workspaces created or removed; silent at startup and when outputs or the compositor appear
- [x] Focus moving to another monitor without a workspace change shows that monitor (`focus` setting); fixes focusing workspace 10, alone on the second monitor, showing nothing
- [x] Indicator: the monitor's workspace dots with the active one stretched into a pill, its name, and a monitor label when two or more monitors are connected (connector name, or a name from `labels`)
- [x] One slot (`key = "workspaces"`), high priority, stacking over the OSD, 1.2 s timeout; fast switching slides the pill in place
- [x] Clicking a dot switches to that workspace; the same action is `mochi ipc workspaces switch <output> <workspace>`
- [x] Settings: `timeout_ms`, `focus`, `urgent`, `changes`, `labels`
- [x] Tested against Hyprland 0.56: switches through the action and by clicking a dot
- [ ] Per-output islands, so the indicator only shows on the monitor that switched (the island is mirrored on every monitor for now)

### OSD

Listens only: it shows changes made anywhere and has no actions.

- [x] Framework: activities carry their `key` to the UI, which updates a keyed replacement in place (the volume bar slides instead of the view reloading)
- [x] Output volume and mute from the default sink, over the PulseAudio protocol (`libpulse-binding`, served by pipewire-pulse), with reconnect and backoff
- [x] Output device switches, with a headset, speakers or display icon
- [x] Microphone mute on the default source
- [x] Caps Lock and Num Lock from the kernel LEDs, polled every 100 ms
- [x] One shared slot (`key = "osd"`), high priority so it interrupts normal activities, 1.5 s timeout
- [x] Silent at startup and after a reconnect
- [x] Settings: `timeout_ms`, and `volume`, `device`, `microphone`, `locks` to turn events off
- [x] Theme-colored line icons drawn in QML
- [ ] Confirm Caps Lock and Num Lock with a physical key press (a virtual keyboard doesn't change the hardware LEDs)
- [ ] Check whether headset dials (Arctis Nova 7) report volume through the audio server
- [x] Laptop screen brightness: in the brightness module, with DDC (polled every 500 ms, as sysfs sends no events for it)
- [ ] Keyboard backlight

### Media

- [x] MPRIS players over zbus: one task follows which players come and go, one task per player reads all its properties again after every change and every seek
- [x] One player per process, so VLC's two bus names show once; playerctld is skipped because it mirrors other players
- [x] The most recently active player wins (last to start playing or change track while playing), and a playing player always beats a paused one; `ignore` hides players by bus name or by the name they give themselves
- [x] A new track takes the island: the expanded view for `expand_ms`, then the compact view for `island_ms`; then the island goes back to what it showed and the music becomes a bubble (cover, title, bars) in `center-left`. Clicking the bubble puts the player back on the island. A pause dims the bubble, which leaves after `paused_ms`
- [x] A track change opens the expanded view for `expand_ms` (the `expand_for` arbiter option): cover, player, title, artist, progress, previous, play or pause, next
- [x] Progress moves in QML from the position and the time it was read, so it stays right when the activity comes back after an interruption
- [x] Click or drag the bar to seek (`SetPosition` with the track id, or a relative `Seek` without one)
- [x] Actions: `play-pause`, `play`, `pause`, `next`, `previous`, `seek <seconds>`
- [x] Tested live with VLC: track changes, pause and resume, seek from the CLI and the bar, the buttons, the player quitting
- [ ] Volume per player

### Notifications

- [x] Serves `org.freedesktop.Notifications` with zbus: `Notify`, `CloseNotification`, `GetCapabilities` (`actions`, `body`, `body-markup`, `body-hyperlinks`, `icon-static`, `inline-reply`, `persistence`), `GetServerInformation`, and the `NotificationClosed`, `ActionInvoked` and `NotificationReplied` signals
- [x] Waits in line for the name behind another daemon (swaync, mako) and takes over when it stops, without taking the name away. zbus's default flags would replace the other daemon and hand the name to the next one; the request uses none
- [x] `replaces_id` updates a notification where it is: the popup in place, or silently in the history
- [x] Popups laid out like the top of the media player: a 64 px icon from the icon theme, the `desktop-entry` hint or the app name, or the picture (`image-path`, or `image-data` pixels written as a PNG to the module's data directory); then the app, the summary and two lines of body. Expanded: app and time, full text, action buttons; clicking the text runs `default`
- [x] A new popup from an app replaces that app's popup in place (a key per app), so a burst shows only the latest and the replaced ones count as missed; `same_app = "stack"` shows each in turn instead. Popups from different apps stack, newest on top. Critical ones never replace or get replaced
- [x] Timeouts from the app or `timeout_ms`; critical ones are urgent, uninterruptible and stay until closed
- [x] Popups that time out go to the history (newest `history` kept), transient ones close. A bell bubble in `center-right` counts them; clicking it lists them on the island with dismiss, clear and do not disturb
- [x] Do not disturb: everything but critical goes straight to the history, and a moon bubble joins the bell
- [x] Actions: `history`, `clear`, `dnd on|off|toggle`, `dismiss <id>`, `invoke <id> <action>`
- [x] A pure `Center` with unit tests for every rule above, and `note.rs` tests for odd hints and padded pixel rows
- [x] Tested live in a private D-Bus session (`dbus-run-session`), leaving the real swaync alone: icons, a critical popup outlasting its timeout, pictures from a path and from bytes, a real click on an action button reaching `notify-send`, replacement, the history and its bubble, do not disturb, clear, and one daemon queueing behind another
- [x] Persist the history across daemon restarts, without the actions
- [x] Body markup (`body-markup`): sanitize to the subset Qt's styled text handles
- [x] Inline replies (`inline-reply`)
- [ ] Sounds (`sound-file`, `sound-name`)

### Launcher

- [x] Framework: `ActivitySpec::modal()`. A modal activity takes the keyboard (`Exclusive`, overlay layer) on the monitor its payload names, and the window covers the screen while it shows, so a click outside the island dismisses it on any compositor
- [x] Desktop entries from `$XDG_DATA_HOME` and `$XDG_DATA_DIRS`, the first id winning, with `NoDisplay`, `Hidden`, `OnlyShowIn`, `NotShowIn` and `TryExec`, translations from the locale, and `[Desktop Action]` groups. Read again on every open, so new installs show up
- [x] Search in the daemon with `nucleo-matcher`: the name counts most, then the generic name, keywords and program; actions match on their own name. Every keystroke is a `search`, and the view ignores answers for older queries
- [x] Most used first: a score per app that grows by one per launch and halves every 30 days, saved to `$XDG_STATE_HOME/mochi/launcher.json`; it orders the empty list and boosts matches
- [x] Starts apps through `uwsm app -- id.desktop[:action]` in a uwsm session, otherwise `systemd-run --user --scope` in `app.slice`, otherwise a detached process group; `Exec` field codes and quoting handled for the last two, terminal apps through `terminal` (default `xdg-terminal-exec`, or `$TERMINAL -e`)
- [x] View like the screenshot: search box, up to seven rows with icon, name and description, the selection marked; arrows or Tab move, Enter starts, Escape closes, hover selects, click starts
- [x] Actions: `toggle`, `open`, `close`, `search [query]`, `launch <id>`
- [x] Tested on Hyprland: typing reaches the launcher, Enter starts a test app through uwsm and records it, it comes first next time, a click outside and Escape close, "private" finds Firefox's and Zen's private window actions
- [x] Keybind examples for Hyprland's Lua config and other compositors (Getting started)
- [x] Calculator and run-a-command results, as providers: `apps`, `calculator`, `commands`, script providers from `config.toml`, and plugin providers (`examples/plugins/emoji`)
- [x] Built-in providers: files (`/`, an index of home), web searches (`!w` and the others), emoji (`:`, the emoji module) and colors (`#`, the colors module)
- [ ] More built-in providers, like open windows or settings pages
- [x] The file index follows changes as they happen (inotify) instead of rebuilding after 15 seconds
- [ ] Watch the application directories instead of reading them on every open, if opening ever feels slow

### Hub

- [x] A module, not part of the framework: `mochi ipc hub toggle|open [page]|close` grows the island into a modal panel, 640 px wide, as tall as its content
- [x] Home: cards from `target = "hub", kind = "card"` contributions in a two-column flow, `options.span` columns wide, each a labeled section on a surface, like a control center; card views give their natural height. Pages: `kind = "page"` contributions as tabs in the bottom navbar, also at their natural height
- [x] Contributed views get their module's published state as `payload`: media publishes the shown player, notifications its history and do not disturb
- [x] Cards: date and time (the hub's own), Now Playing (media, two columns, with progress and controls), the latest missed notifications. Page: the notification history
- [x] The hub and the launcher close each other on open through `ctx.call`, ignoring `NotEnabled`
- [x] Look, from the controls gallery: no page title, small section labels with an icon, tight spacing, a divider, and a navbar of icon pills like the workspace dots, the current one stretched into a white pill with its name
- [x] `Symbol`: a built-in icon set in the core (home, bell, music, clock, grid, moon, volume, power, lock, logout, reboot, snow, chip, leaf, bolt, scale), filled or stroked, so contribution icons look the same everywhere; other names come from the icon theme
- [x] Tested in a private D-Bus session: the three cards, the notifications page opened with `open notifications/history`, and the launcher replacing the hub
- [ ] Cards spanning two rows, like the tall Now Playing tile in the inspiration
- [x] Three columns, cards packed densely with the last of a row stretched over free columns, cards that hide when empty, a height capped to the screen with scrolling and the navbar always shown
- [x] Cards and pages for network and Bluetooth
- [x] A Performance page: CPU, memory and GPU with graphs and the busiest processes
- [ ] More cards and pages: power
- [x] Clicking a card opens its page: the heading or the card beside its controls, `options.page` to pick

### Power

Lives only in the hub: no island view, a page and the CLI.

- [x] logind over the system bus: `PowerOff`, `Reboot`, `Suspend`, `Hibernate`, and reboot to firmware through `SetRebootToFirmwareSetup`, all interactive so polkit can ask for a password. A button shows only when its `Can…` answer is `yes` or `challenge`
- [x] Lock and log out act on the user's graphical session from `User.Display`, since a daemon often has no `XDG_SESSION_ID`: logind locks it (the locker answers), log out is `uwsm stop` under uwsm or ends the session. `lock` and `logout` settings replace either with a command
- [x] Power profiles through power-profiles-daemon (`org.freedesktop.UPower.PowerProfiles`), the active one followed live; hidden when the service doesn't run
- [x] Page: one tile per action, and a segmented control for profiles with the active one as a white pill. Log out, reboot, firmware and shut down need a second click within 3 seconds; acting closes the hub
- [x] Actions: `lock`, `logout`, `suspend`, `hibernate`, `reboot`, `firmware`, `shutdown`, `profile <name>`
- [x] Tested on this machine: hibernate hidden (logind says `na`), switching to performance and back from the CLI moves the selector, and two real clicks on Log out with a harmless replacement command (the first only asks)
- [x] The older `net.hadess.PowerProfiles` name, for power-profiles-daemon before 0.20
- [ ] A lock screen of its own, if hyprlock and the others ever fall short (see open question 3)
- [ ] Reboot into another OS, only once it can work with any bootloader and distro. Today no single way does: logind's one-shot boot loader entry only covers loaders that follow the Boot Loader Interface (systemd-boot), GRUB needs root to run `grub-reboot`, and the firmware's `BootNext` needs root too. Decided on 2026-10-03 to wait

### Capture

Screenshots and recordings from the island. Decided on 2026-10-04: our own frozen overlay instead of slurp, gpu-screen-recorder for recording, and a preview card on the island afterwards.

- [x] Prototype: a frozen `ScreencopyView` frame per monitor on Hyprland with NVIDIA, saved at full resolution with `grabToImage` in under a second. No grim fallback needed
- [x] Framework: `ActivitySpec::overlay(view)` draws a view over every monitor under the island; the island waits for the overlay's `ready`, so a frozen frame never shows the picker. The island window now ignores reserved space and `EdgeReserve` keeps windows away, so an overlay covers the real screen even under another bar
- [x] Actions: `screenshot [region|window|screen]`, `record [region|window|screen]`, `mode`, `stop`, `cancel`, and `copy`, `edit`, `delete`, `open` for the last capture. The picker opens in the `mode` setting (region by default) so a drag works right away; the island is a bar with the three modes, the current one highlighted, to switch with a click, Tab or 1 to 3, M for the microphone, Escape to cancel
- [x] Overlay: a region drawn by dragging, moved by dragging inside, resized by its corners, with its size and a Capture button; the window under the pointer outlined with its title; a whole screen by a click. Right click cancels
- [x] Only the picked monitor's frame is saved, after the pick and as raw PPM (about 25 ms), so the opening animation never stalls; the module compresses the PNG off the event loop with `png`'s fast level (a full 1080p screen in about 0.2 s, a region in 60 ms)
- [x] The module cuts the picked area out of the saved frame, mapping logical coordinates to buffer pixels per monitor with edges rounded outwards; unit-tested for scales 1, 1.25 and 1.5 and for outputs to the right
- [x] Window geometry from `mochi-compositor`'s `windows()`, from Hyprland's `clients` (its `visible` flag, or the monitors' active workspaces on older versions), topmost first; Window is hidden for screenshots elsewhere
- [x] Recording through `gpu-screen-recorder`: a region, a monitor by name, or a window through the portal; desktop audio, and the microphone mixed in when asked. A breathing red dot bubble (a timer with `wide = true`); clicking it, `stop` or `record` sends SIGINT so the file is finished. Its usual failures are explained on the island
- [x] After a capture: saved to `$XDG_PICTURES_DIR/Screenshots` or `$XDG_VIDEOS_DIR/Recordings`, copied with `wl-copy` (recordings as a file), and a card with a thumbnail and Copy, Edit (`satty --filename`), Open folder and Delete
- [x] Settings in a checked `settings.toml`; docs page with the NixOS note for gpu-screen-recorder
- [x] Tested on Hyprland with two monitors: a full screen, a region on the left monitor and a window on the right one cropped to the exact sizes, a real hover outlining a window and a real click taking it, the recording bubble stopped by a real click, and the failure card when gpu-screen-recorder lacks its capture helper. Recording itself ran with a stand-in recorder, since this machine doesn't enable the helper
- [x] Picking on another monitor than the one the picker opened on: every overlay asks for the keyboard, because Hyprland only sends the pointer to the surface holding it; tested with a real click on HDMI-A-1 after opening on DP-3
- [ ] Try keyboard use in the picker (arrows, Enter, Escape, M) on a real session; the tests here only used the pointer and the CLI
- [ ] Record a real video once `programs.gpu-screen-recorder.enable` is on
- [x] Window positions on other compositors: niri and Sway IPC
- [ ] A thumbnail for recordings
- [x] A region across several monitors: regions are global, the first overlay sends the layout, a drag keeps going into the next screen and the other screens draw their part as it moves; each screen it touches saves its frame and `crop::join` puts them together at the finest scale, transparent where no screen is. Tested on Hyprland through IPC; a real drag across still needs trying by hand. Recordings stay on one screen
- [ ] Recording a region across screens, if gpu-screen-recorder can
- [x] Capture every screen at once: the All screens mode, `mochi ipc capture screenshot all`
- [x] A hub page for captures: the latest screenshots and recordings as a history, with thumbnails, and the preview card's copy, edit, open folder and delete on each
- [ ] Thumbnails for recordings: a frame from each video, made once and kept
- [x] Pick the quality of a recording and of a screen share: frame rate presets (15, 30, 60, 90, 120 fps) and resolution presets (480p, 720p, 1080p, 1440p), in the picker and as settings. Recordings pass gpu-screen-recorder `-f` and `-s`; a switchable share sets `MOCHI-SHARE`'s refresh rate and size. A share that isn't switchable stays the app's to choose

### Share

After capture, reusing its pickers.

- [x] `mochi share-pick` is xdg-desktop-portal-hyprland's `custom_picker_binary` (packaged as `mochi-share-picker`, which the home-manager module writes into `xdph.conf`). The island panel shows screens and the portal's windows with live thumbnails, Hyprland toplevels matched by address, plus Region over a live overlay and a Remember switch; the answer goes back as the command's new `output` reply. Without Mochi or the module it runs `hyprland-share-picker`
- [x] A bubble while the screen is shared, from Hyprland's `screencast` events counted per capture session; it waits 1.5 s and ignores the picker's own thumbnails, since Hyprland reports every capture, screenshots included
- [x] Clicking the sharing bubble asks again what to share, for the app sharing now. The portal can't change a running share, so with Switchable on (the default) the app shares `MOCHI-SHARE`, a Hyprland headless monitor far from the real ones, and Mochi draws a live copy of the screen, window or region on it; the bubble's picker changes the copy. The monitor goes 3 s after `screencastv2` stops reporting captures of it
- [x] Keep the switchable copy's aspect: size `MOCHI-SHARE` to the first source instead of the largest screen (screens and areas; a window keeps the largest screen's)
- [ ] `chooser_cmd` for xdg-desktop-portal-wlr
- [x] Screencast state on niri, from its casts
- [ ] Screencast state on Sway and others (PipeWire streams)

### Clipboard

- [x] Watch the clipboard through `ext-data-control-v1` on a Wayland connection of the module's own: the best text format with HTML alongside, or an image, PNG first. Skip `x-kde-passwordManagerHint` and Mochi's own selection, marked with `application/x-mochi-clipboard`
- [x] An append-only log with sealed info and content parts, read into an index at startup without the content; XChaCha20-Poly1305 with a key from the Secret Service on disk, a BLAKE3 checksum in memory; zstd for text; dedup by hash; a rewrite on removal
- [x] In `$XDG_RUNTIME_DIR` by default, through the new `ModuleCtx::session_dir`; `storage = "disk"` falls back to memory without a key
- [x] The picker on the island, like the launcher, with image thumbnails; Enter pastes through `zwp-virtual-keyboard-v1` with a keymap of its own, Ctrl+Shift+V for terminals from the compositor's `focused_app`
- [x] A hub card: count, pause, clear
- [x] A hub page: the history with search, paste, copy, delete, pause and clear
- [x] Tested end to end: copies, secrets skipped, a restart, disk mode against a private gnome-keyring, and pasting into `wev` in a headless Sway
- [ ] `wlr-data-control` for compositors without the ext protocol
- [x] Pinned entries
- [x] Ignored apps, by the focused window's app id, the password manager hint, and a pause
- [x] A larger preview of the selected image or long text
- [x] Clicking an image entry, in the picker or the hub page, opens it in the capture module's preview card through its new `show` action: copy and edit as for a screenshot, no folder, and delete removes the entry
- [ ] Check the paste on Hyprland with a non-QWERTY layout

### Audio

A new module, `audio`: the volume mixer. Players stay in the media module and notices in the OSD.

- [x] Switch which player the island shows when several have a track, like Spotify and a browser video: arrows next to the player's name on the island and the hub card, and `next-player`, `previous-player` and `player <name>` in the media module. The pick holds until that player stops or closes
- [x] Volume mixer: a volume slider and mute per app (PulseAudio sink inputs, served by pipewire-pulse), next to the output and input volumes, keeping each device's balance
- [x] Output and input device switch
- [x] The mixer is both a hub page (Sound) and an island view (`mochi ipc audio toggle`), with `volume`, `mute`, `output` and `input` actions for keybinds
- [x] Fleeting activities: the volume and workspace notices never queue behind a panel, so they don't show late once it closes
- [x] Move an app to another output, like Discord on the headset and Spotify on the speakers
- [ ] Recording apps: a slider per app using the microphone (source outputs)
- [x] Group an app's streams into one row, with a way to open them
- [x] Peak meters next to the sliders
- [ ] App icons in the mixer: Chromium, Zen and WebRTC streams name icons the theme lacks; look the app up by its desktop entry or process instead
- [x] Scroll on the OSD to change the volume (there's no volume bubble)

### Tray

A new module: the apps' tray icons.

- [x] StatusNotifierItem over D-Bus: register as the watcher and host, follow items as they come and go. When another watcher runs, show its items instead
- [x] Icons and attention state, from the icon theme, the app's own icon folder, or the pixmaps the app sends
- [x] Click to activate, right click for the app's menu (DBusMenu) in Mochi's style with submenus as pages, middle click, scroll
- [x] Where it lives: a tray bubble that opens a drawer on the island, and a bubble of their own for apps in `pinned`. Attention makes the bubble breathe
- [ ] Tooltips on hover in the drawer and on pinned bubbles
- [ ] Keyboard in the drawer and the menus: arrows, Enter
- [ ] Follow a menu's changes while it's open (`LayoutUpdated`)
- [ ] XEmbed tray icons, through a bridge like xembedsniproxy

### Network

- [x] NetworkManager over D-Bus: devices, Wi-Fi networks, active and saved connections, read again after its signals
- [x] A bubble with the connection, a Network page, a home card, notices on connecting and disconnecting
- [x] Join Wi-Fi with a password asked on the island, forget networks, VPNs, airplane mode
- [ ] WPA Enterprise: ask for the user name and password too
- [ ] Hidden networks
- [ ] Be NetworkManager's secret agent, so a changed password is asked on the island
- [ ] Mobile broadband

### Bluetooth

- [x] BlueZ over D-Bus: the adapter, devices and batteries, read again after its signals
- [x] A bubble with the connected device's battery, a page, a home tile, notices on connecting and disconnecting
- [x] Scan, pair through Mochi's agent with the questions on the island, connect, forget
- [ ] Battery of devices that report it only through their own app, like some headsets
- [ ] More than one adapter

### Battery

- [x] UPower's display device: the level, charging, time left
- [x] Notices when dropping past configurable levels, once per discharge; a warning bubble at or under a level, red at the critical one; notices on plugging in or out; a hub card
- [x] Power profiles on the card, next to the power module's
- [ ] Each battery on its own, and peripherals' batteries (mice, controllers) from UPower

### Performance

- [x] CPU use and temperature, memory and swap, GPU use, video memory and temperature (AMD from sysfs, NVIDIA from one long-running nvidia-smi)
- [x] Notices when a reading stays over a level, naming the busiest process; a red bubble while one stays critical; a hub page with graphs and the busiest processes
- [ ] Intel GPUs
- [x] Disk and network throughput
- [x] End a process from the page

## Plugins

- [x] `mochi-plugin.toml`: `[plugin]` (`id`, `api`), `[backend]`, `[release]`, `[views]` with `overrides`, `[uses]`, `[[actions]]`, `[[contributions]]`
- [x] `plugins.toml` next to config.toml, with `git:`, `git-release:` and `path:` sources, and `plugins.lock`
- [x] Report missing plugins and invalid manifests in `mochi status` and `mochi plugins list`
- [x] Reject plugins with an unsupported `api` version, or a builtin's id
- [x] QML-only plugins: their views go in the shell, linked
- [x] View overrides (`overrides = ["idle/Pill"]`) with fallback to the builtin view
- [x] Backend plugins: spawned with a socket pair on fd 3, supervised with backoff, wrapped in the `Module` adapter
- [x] Plugin actions reachable through `mochi ipc <plugin> <action>`
- [x] Restart a plugin whose files changed on `mochi reload`
- [x] `mochi plugins install|update|remove|list`, showing what a plugin runs and asking first
- [x] `mochi-sdk` crate
- [x] Example plugins: pomodoro and weather
- [x] Plugin docs: Plugins and Writing plugins pages, the plugin protocol in `docs/protocol.md`
- [x] home-manager `plugins` option
- [ ] Publish `mochi-sdk` to crates.io
- [ ] `git-release:` for Forgejo, Gitea and GitLab
- [ ] Archive sources (`https://…/plugin.tar.gz` with a hash)
- [ ] A plugin replacing a builtin module entirely
- [x] Building plugins with Nix, for declarative setups: home-manager's `plugins.<id>.src` and `package`, `lib.buildPlugin`, and flakes in `mochi plugins install`
- [x] `lib.buildPlugin` for plugins in other languages than Rust: Node, Python, Go with vendor/, scripts and release archives
- [ ] Go plugins without vendor/, and Java, in `lib.buildPlugin`: both need a hash Nix can't get from their lock files
- [ ] Static musl binaries in `examples/plugins/release.yml`, so `git-release:` plugins run on NixOS without nix-ld
- [x] Release workflow template for plugin repositories: `examples/plugins/release.yml`
- [x] Settings checks for plugins, from their `settings.toml`

## Widgets

Views from any module on the desktop, under the windows, placed by dragging. Designed on 2026-10-06: a `widgets` module owns `widgets.toml` next to `config.toml`; modules offer widgets with `widgets` contributions; instances have ids and settings; positions are an anchor and grid cells per monitor; a desktop layer per monitor in the core.

- [x] `widgets.toml`: instances with module, widget, output, anchor, offsets and size in cells, settings; watched and applied live, a broken file keeps the last layout; checked by `mochi config check`
- [x] Widget contributions with sizes, limits, frame, declared settings and `forget`
- [x] Desktop layer per monitor: widgets under the windows, cards with shadow and blur, clicks only on widgets, keyboard on demand for a typing widget
- [x] Edit mode: dim and grid, drag to move, corner to resize, generated settings form, drawer to add, remove, Copy as Nix, Escape
- [x] Clock widget: time zone, 12/24 hours, seconds, date
- [x] `widgets export|copy [nix|toml]`
- [x] home-manager `programs.mochi.widgets`, written only when the declared layout changes, taking Nix or TOML
- [x] Calendar widget
- [x] To-do and notes widgets, typed into on the desktop (the notes module)
- [x] Widgets from existing modules: now playing, performance graphs, battery, weather
- [x] Alignment guides while dragging
- [ ] Moving a widget to another monitor from its settings

## Emoji and colors

- [x] Emoji module: grid panel with search, groups and recents; paste or copy through the clipboard module; `:` provider
- [x] Colors module: screen picker with a magnifier, exact pixels from wlr-screencopy at any scale and rotation; island card with HEX, RGB, HSL and OKLCH; history in a hub card and page; `#` provider
- [x] Skin tones in the emoji grid
- [ ] Colors: a palette widget for the desktop

## Polish and features

### Frontend

- [x] A design system in `Theme`: a type scale (four sizes and display), a spacing scale, radii for surfaces, fields and controls, heights, Inter and Material Symbols; the core controls and the hub use it
- [x] Every module's views moved onto the scale, emptying `crates/mochi-core/tests/design-baseline.txt`
- [x] A check in `nix flake check` that rejects raw pixel sizes and colors in module QML, so views can't drift from the scale again (`crates/mochi-core/tests/design.rs`)
- [ ] The hub's Home as a control center: tiles of one height with their label inside, slider tiles (volume, brightness) with a `›` to their page, and an icon-only footer instead of the labelled tab
- [x] The hub sized to its content, its outline animating between pages instead of one fixed size for all of them
- [ ] An editable control center: choose the tiles and drag them into place, reusing the widgets editor's drag and snap
- [x] Shared components (now to be used by every module): a panel header (back, title, actions), fading edges where a list scrolls, rolling digits for the clock and percentages, a light along a panel's top edge that follows the pointer and pulses while something works, switch rows and slider rows
- [x] Theme presets: named palettes in a picker with preview cards, applied live
- [x] A light mode for every preset (replaces "Light variant of the palette" under Design system)
- [x] Colors from the wallpaper, with contrast corrected without moving the hue (replaces "Colors generated from the wallpaper" under Design system)
- [x] Motion settings: reduced motion, which turns animations off, and a speed multiplier for the rest
- [ ] A one-line launcher layout: icon and name, the description only on the selected row

### Features

- [ ] Drop files on the island: it says what was dropped and offers actions that fit, like compress, merge PDFs, convert images, extract an archive or open with
- [ ] Coding agents' status on the island: working, waiting for you, or done, for T3 Code and Claude Code through their hooks
- [x] A privacy indicator: a dot on the island while the microphone or camera is in use, with a mic mute; the audio module already sees recording apps
- [x] Keep awake: a toggle that blocks idle and sleep
- [x] Night light, through wlr-gamma-control itself rather than hyprsunset or wlsunset
- [ ] A focus timer, built in (the pomodoro example plugin shows the idea)
- [x] Brightness: the laptop's backlight and external monitors over DDC/CI with ddcutil, with the OSD and a slider tile (replaces "Laptop screen brightness" under OSD)

### Checks and tools

- [x] A test that loads every QML view in a temporary shell, part of `nix flake check`, so a broken view fails the build
- [x] `mochi doctor`: checks the compositor's protocols, the portals, the fonts and the tools modules and plugins need, and says what to install

## Integration and docs

- [x] Session setup notes: uwsm and `graphical-session.target`, or starting `mochid` from the compositor's autostart (README)
- [x] Keybind examples for `mochi ipc`: Hyprland's Lua config, `hyprland.conf`, Sway and niri, on the Getting started page
- [ ] Decide how the island and Waybar share the top edge, or whether Mochi replaces Waybar
- [x] Install instructions (Nix package and cargo)
- [x] Flake `packages` output: both binaries, the systemd unit, and `mochid` wrapped with the pinned Quickshell
- [x] `nix flake check` builds the package and runs the test suite
- [x] End-to-end daemon tests with a fake Quickshell (run in the Nix build)
- [x] Recording test against the real binaries (`cargo test -p mochid --test record -- --ignored`), writing outside the repository
- [x] NixOS and home-manager modules and an overlay in `packaging/nix`. The home-manager module writes `config.toml` and `theme.toml` from Nix, checks them with `mochid config check` at build time, and reloads instead of restarting when they change
- [x] Arch packages in `packaging/arch`: `mochi` from the release tarball and `mochi-git` from `main`, both built and installed in an Arch container
- [ ] Publish `mochi` and `mochi-git` to the AUR; bump `pkgver` and `sha256sums` in `mochi` with each release
- [x] Release 0.0.1 and `CHANGELOG.md`
- [x] Release 0.0.2: capture, overlays, gpu-screen-recorder in the Nix package and modules
- [x] Release 0.0.3: the frozen screen no longer stretches when a screenshot opens
- [x] Release 0.0.4: clipboard, share, Arch packages, screenshots over anything
- [x] Release 0.0.5: plugins and the SDK, audio mixer, tray, network, Bluetooth, battery, performance, stacked bubbles
- [x] Release 0.0.6: widgets and notes, emoji and colors, launcher providers, notices on one monitor, `mochi dismiss`
- [x] Release 0.0.7: the settings panel, shares that survive a restart, notification markup and replies, the mixer's apps and meters, plugins built by Nix, the design scale
- [x] Documentation site with mdBook in `docs/book`: installing, getting started, configuration, bubbles, theme, a page per module that includes its `settings.toml`, writing views and the protocol. `nix build .#docs`, part of `nix flake check`; `.github/workflows/docs.yml` publishes it to GitHub Pages
- [ ] Publish the site once the repository is on GitHub
- [ ] `docs/architecture.md`

## Later

- [ ] Lua or WASM plugin backends
- [ ] SDKs for other languages
- [x] Settings UI: the settings module
- [x] A guided tour, and a tour of what's new after each release: the tour module

## Open questions

1. Multiple monitors: one island per output, or only on the focused output?
2. ~~Can a plugin read state from other modules?~~ Yes: `[uses] state` in the manifest, and `ModuleCtx::watch_state` for builtins.
3. Is the lock screen a module, or a separate minimal program? A crash in the lock screen is a security problem.
4. Does the theme control the animation springs, or are they fixed per view?

## Suggested order

Done: the spike, phase 1 (protocol, core, daemon, CLI, QML core, the idle module and packaging), the compositor adapter, the layout system, bubbles, and the OSD, workspaces, media, notifications, launcher, hub and power modules.

1. Capture: screenshots and recordings
2. Share: the portal picker and a sharing bubble
3. Hub pages: audio, network, Bluetooth
4. ~~Plugins~~ (v1 done)

# Changelog

Mochi follows [semantic versioning](https://semver.org). Before 1.0, any minor release may change the config format, the protocol or the module interface; the changelog says when.

## Unreleased

### Added

- Screenshots across monitors: a region can run from one screen into the next, and the screenshot joins their parts at the sharpest screen's scale, transparent where no screen is. A new All screens mode, `mochi ipc capture screenshot all`, takes every screen as one image.
- `[island] panels` in `config.toml`: the launcher, the hub, the clipboard and other views you type into open on the monitor with keyboard focus (the default), the one under the pointer, or all of them. The protocol's activity has a new `output` field.
- `[island] click_outside`: a click outside the island now closes whatever it shows, a notification popup, the media card, a screenshot's preview, instead of only views you opened. A popup you never opened goes to the missed ones. The volume and workspace notices let clicks through (`ActivitySpec::passive`). `"expanded"` goes back to closing only views you opened. The protocol has a new `outside` field and event.
- Clicking a clipboard image opens it in the screenshot preview card, with copy, edit and delete. The capture module has a `show` action for it.
- Audio module: a volume mixer, as the hub's Sound page or on the island with `mochi ipc audio toggle`. It has the output and the input with their volumes and a list of devices to switch to, and a volume and mute for each app playing sound. `volume <target> <level>` (with `+5` and `-5`), `mute`, `output` and `input` do the same from keybinds. `max_volume` sets the loudest the sliders and `volume` go, up to 300%, and the OSD's volume bar follows it, with a mark at 100%. It's on in newly generated configs.
- Media: arrows next to the player's name, on the island and the hub card, switch between players when several have a track, like Spotify and a browser video. The one you pick stays shown until it stops, even when another starts a new track. `next-player`, `previous-player` and `player <name>` do the same from keybinds.
- Share: clicking the sharing bubble changes what you share, without the app asking again. The portal can't change a running share, so with the picker's new Switchable switch on, the default, the app shares a monitor of Mochi's own, `MOCHI-SHARE`, and Mochi draws a live copy of the screen, window or region you picked on it. The bubble, or `mochi ipc share switch`, opens the picker to change the copy. The monitor goes a few seconds after the app stops. `switchable = false` in `[module.share]` shares the choice itself, as before. Hyprland only.
- Quality presets for recordings and switchable screen shares: the pickers step the frame rate through 15, 30, 60, 90 and 120 fps (F in the capture picker) and the resolution through Native, 480p, 720p, 1080p and 1440p (Q), which scales down keeping the shape and never up. Recordings pass them to gpu-screen-recorder; a switchable share makes its monitor at that size and refresh rate. `resolution` is a new setting next to `framerate` in `[module.capture]`, and both are new in `[module.share]`. `mochi_core::quality` has the presets.
- Views: a module can show `Screen.qml` on a monitor of its own, named `MOCHI-<MODULE>`. `Daemon.screens` lists the real monitors and `Daemon.virtualScreens` these. The compositor state has each output's size, and what is being captured (`captured`, from Hyprland's `screencastv2` events).
- `ActivitySpec::fleeting`: an activity that shows at once or not at all, never queued or suspended. The volume and workspace notices are fleeting, so they no longer show late after the hub, the launcher or the mixer closes. The volume notice now replaces a workspace notice on screen instead of waiting for it.

### Fixed

- Stopping mochid gives modules up to 2 seconds to clean up, so the share module removes its switchable monitor instead of leaving it behind.
- A module added by `mochi reload` showed nothing when its views use each other, like the audio mixer: Quickshell only finds a directory's QML types when it starts. The daemon now restarts Quickshell when a reload adds a module.
- Each monitor's island sends events for the activity it shows, not the daemon's current one.
- The capture overlay no longer reports itself ready before it knows its screen, which could let the island change before the screen froze.

## 0.0.4 - 2026-10-05

### Added

- Clipboard module: a clipboard history on the island. `mochi ipc clipboard toggle`, bound to SUPER+V, searches what you copied, text and images with thumbnails; Enter pastes the entry into the window you were in by typing Ctrl+V, or Ctrl+Shift+V in a terminal, and Shift+Enter only copies it. It reads the clipboard through `ext-data-control-v1` and types through `zwp-virtual-keyboard-v1`, so nothing else needs to run. The history is a log file of its own, in the runtime directory until you log out, or with `storage = "disk"` in `$XDG_STATE_HOME/mochi`, encrypted with XChaCha20-Poly1305 under a key kept in the Secret Service. Copies password managers mark as secret are never read, and removing an entry rewrites the file. A hub card shows the count and pauses or clears the history. It's on in newly generated configs.
- The hub has a Clipboard page: the history with a search box, pause and clear; a click pastes an entry, and each row can copy or remove it.
- `Priority::TOP`, which interrupts any activity, even an uninterruptible one.
- `ModuleCtx::session_dir`, a module directory in the runtime directory that survives a daemon restart, unlike `data_dir`.
- The compositor state has `focused_app`, the app id of the window that had focus last.
- Share module: the screen-share picker for xdg-desktop-portal-hyprland. The island shows the screens and windows with live pictures, or lets you draw a region, with a Remember switch; `mochi share-pick` is the program the portal runs, packaged as `mochi-share-picker`, and the home-manager module writes `xdph.conf` for it (`portalPicker.enable`). A bubble shows while the screen is shared. It's on in newly generated configs.
- Commands can answer with output, a new `output` message, which `mochi ipc` prints.
- The compositor state says when the screen is captured, from Hyprland's `screencast` events.
- Recording can leave out the desktop audio: a speaker toggle next to the microphone one in the picker, the A key, the `audio` action, and `record_audio` for the default.
- Arch Linux packages in `packaging/arch`: `mochi` builds the latest release, `mochi-git` the latest commit. Until they are on the AUR, the installing page shows how to build them from a clone with `makepkg -si`.
- A hairline border and a soft shadow around the island and the bubbles, as the theme colors `border` and `shadow`; transparent turns either off. The border skips any edge the island is attached to in notch mode.

### Changed

- Screenshots and recordings open over anything on the island, the launcher, the hub or the clipboard included, instead of waiting for it to close. The frozen screen still shows it, so the shell itself can be captured, and it comes back afterwards.
- A notification or the media card you expanded with a click now takes the keyboard: Escape or a click outside closes it. Views that open on their own never take the keyboard.

### Fixed

- Apps started from the launcher no longer close when Mochi stops. uwsm gave them a scope of their own, but they stayed children of mochid, and an app that ends with its parent, like Discord in bubblewrap with `--die-with-parent`, closed with it. Apps, the screenshot editor, the folder opener and the power commands now start through a double fork, adopted by systemd.
- A light 1px line around the island and the bubbles: their blur reached past the drawn outline. It now stays a pixel inside.

## 0.0.3 - 2026-10-04

### Fixed

- Opening a screenshot no longer stretches the frozen screen. The island's window is now always as tall as the screen, so the compositor never animates a resize: Hyprland's `layers` animation stretched it while the window grew. `layout.surface_height` now caps the island itself.
- Recording works when the GPU's hardware encoder is unusable, as with NVENC on a GTX 1060, whose last driver (580) is older than the NVENC version nixpkgs' FFmpeg needs. The module asks `gpu-screen-recorder --info` which codecs work and picks a Vulkan one when no hardware codec is listed, with CPU encoding as the last resort. The `codec` setting forces one. Regions use gpu-screen-recorder's `-w WxH+X+Y`, as `-region` is deprecated.

## 0.0.2 - 2026-10-04

### Added

- Capture module: screenshots and recordings from the island. `mochi ipc capture screenshot` freezes the screens and opens in a default mode, a region unless set otherwise, with the island switching to a window or a screen; the capture is saved, copied, and shown with buttons to copy, edit, delete or open its folder. A screenshot takes under a tenth of a second after the pick. `record` records through gpu-screen-recorder, with a red dot next to the island while it runs; when it can't start, the island says why. It's on in newly generated configs.
- Overlays: an activity can draw a full-screen view on every monitor, under the island, with `ActivitySpec::overlay`. The protocol's activity has a new `overlay` field.
- Window positions from Hyprland's IPC, for modules that pick a window on screen.

### Changed

- The island's window no longer reserves the edge itself: a separate invisible strip does. The island now always starts at the screen edge, even with another bar there, and can cover the screen without moving windows.
- The Nix package brings gpu-screen-recorder, unless the home-manager settings name another recorder. The NixOS module also enables `programs.gpu-screen-recorder`, which recording a region or a screen needs.

### Fixed

- The Nix package's description, which broke its build in the commit after 0.0.1.

## 0.0.1 - 2026-10-04

The first release.

### The shell

- `mochid`, the daemon. It owns all state, runs the modules, and starts and supervises Quickshell 0.3.1. It restarts the UI if it crashes.
- `mochi`, the command-line client: `status`, `reload`, `ipc <module> <action>` and `config`.
- The island, at the top or bottom edge, sizes itself to what it shows and animates between views. Clicking expands or collapses it; right-clicking closes it.
- The island picks what to show by priority. A new activity can interrupt or queue behind the current one, and replaces any earlier one with the same key.
- Island and notch layouts. The notch touches the screen edge and has curved ears.
- Bubbles: small round status items in five areas along the edge. Modules choose where they go; `[bubbles.<module>]` in `config.toml` moves them, groups them or makes them wide pills.
- Modules can add cards and pages to other modules, such as the hub, and call each other's actions.
- The Obsidian theme, with built-in controls for views: buttons, sliders, switches, tiles, list rows and icons.

### Modules

- Idle: the clock, shown when nothing else is. A click opens the hub.
- OSD: volume, mute, the output device, microphone mute, Caps Lock and Num Lock, from PipeWire or PulseAudio.
- Workspaces: the workspace indicator, from `ext-workspace-v1`, on any compositor that supports it.
- Media: what's playing in any MPRIS player, with controls. A new track shows the player, then the music becomes a bubble.
- Notifications: a freedesktop notification server. A new message from an app replaces its previous one. Missed notifications become a bubble, and do not disturb silences them. Mochi waits behind swaync, mako or dunst and takes over when they stop.
- Launcher: fuzzy app search, most used apps first, and app actions. Starts apps through uwsm or a systemd scope.
- Hub: a panel with pages and cards from other modules.
- Power: lock, log out, suspend, hibernate, reboot, reboot to firmware and shut down, plus power profiles. It shows as a hub page. Actions that end the session ask for a second click.

### Configuration

- The first start writes a commented `config.toml` and `theme.toml` with every option and its default.
- `mochi config init`, `check` and `path`. Errors name the file, the section and the key.
- `mochi reload` applies both files without a restart. It starts, stops or restarts only the modules that changed, and keeps the running config if the new one has an error.

### Packaging

- A Nix flake with the package, an overlay, a home-manager module and a NixOS module, in `packaging/nix`. The home-manager module writes the config from Nix, checks it at build time, and reloads the shell when it changes.
- A systemd user unit, `mochid.service`, bound to `graphical-session.target`.
- A documentation site, built with mdBook from `docs/book`.

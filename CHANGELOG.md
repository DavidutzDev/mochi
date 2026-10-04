# Changelog

Mochi follows [semantic versioning](https://semver.org). Before 1.0, any minor release may change the config format, the protocol or the module interface; the changelog says when.

## Unreleased

### Added

- Recording can leave out the desktop audio: a speaker toggle next to the microphone one in the picker, the A key, the `audio` action, and `record_audio` for the default.

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

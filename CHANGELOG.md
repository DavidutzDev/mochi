# Changelog

Mochi follows [semantic versioning](https://semver.org). Before 1.0, any minor release may change the config format, the protocol or the module interface; the changelog says when.

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

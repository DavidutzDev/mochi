# Mochi

A desktop shell built around a central island, like a dynamic island. `mochid`, a Rust daemon, owns state and system integration and supervises a Quickshell UI. `mochi` is its command-line client.

Early work in progress, at version 0.0.7: the island, bubbles, the notch layout, plugins, and the idle, OSD, workspaces, media, audio, notifications, launcher, hub, power, capture, share, clipboard, tray, network, bluetooth, battery, performance, widgets, notes, emoji, colors and settings modules work today. `CHANGELOG.md` lists what each release has. `TODO.md` has the plan, the documentation site in `docs/book` how to install and configure it, `docs/protocol.md` the daemon's protocol and the plugin protocol, `docs/views.md` how to write views with the built-in controls and `docs/spike.md` the results of the first prototype.

## Running it

Everything runs from the dev shell (`nix develop`, or `direnv allow` once).

```sh
cargo run -p mochid -- --dev --modules idle,osd,demo
```

`--dev` links the QML to the source tree, so editing any `qml/` file updates the running island. `--modules` overrides the module list in `~/.config/mochi/config.toml`. Stop it with Ctrl+C.

From another terminal:

```sh
cargo run -q -p mochi -- status
cargo run -q -p mochi -- ipc                      # list every action
cargo run -q -p mochi -- ipc demo show Card "Some text"
cargo run -q -p mochi -- ipc demo alert Small     # interrupts the card
cargo run -q -p mochi -- ipc demo volume 40       # replaces itself in place
```

On the island, a left click expands or collapses the card and a right click closes it. Hovering pauses the timeout.

Logs go to stderr. `MOCHI_LOG=debug` shows every activity change.

## Installing

### Nix

The flake has the `mochi` package, a home-manager module, a NixOS module and an overlay. With home-manager:

```nix
# flake inputs
mochi.url = "github:DavidutzDev/mochi/v0.0.7";

# home configuration
imports = [ inputs.mochi.homeModules.default ];
programs.mochi = {
  enable = true;
  settings.module.idle.format = "HH:mm:ss";   # config.toml, as Nix
  theme.layout.mode = "notch";                 # theme.toml
};
```

This installs `mochid` and `mochi`, starts the shell with the graphical session, and checks the config at build time. The modules live in `packaging/nix`, and the documentation site's installing page covers every option.

### Arch Linux

`packaging/arch` has `mochi`, the latest release, and `mochi-git`, the latest commit. Until they're on the AUR, build one from the repository:

```sh
git clone https://github.com/DavidutzDev/mochi.git
cd mochi/packaging/arch/mochi-git    # or mochi, for the latest release
makepkg -si
systemctl --user enable --now mochid
```

The installing page lists the optional dependencies and the share picker's setup.

### Other systems

```sh
cargo build --release
sudo install -m755 target/release/mochid target/release/mochi /usr/bin/
install -Dm644 systemd/mochid.service ~/.config/systemd/user/mochid.service
systemctl --user enable --now mochid
```

`mochid` needs Quickshell 0.3.1 in its `PATH` and refuses to start with another version.

Workspace information comes from the standard `ext-workspace-v1` Wayland protocol, so it works on any compositor that supports it, with no compositor-specific setup. The focused monitor comes from the focused window (`wlr-foreign-toplevel-management`); on Hyprland, its event socket makes that exact. `mochi status` shows what the daemon found.

### Session

The unit starts with `graphical-session.target`. Session managers such as uwsm start that target and import `WAYLAND_DISPLAY` and `HYPRLAND_INSTANCE_SIGNATURE` into the user manager. Without one, start `mochid` directly from your compositor's autostart instead; it doesn't need systemd.

Logs are in `journalctl --user -u mochid`, and `systemctl --user reload mochid` runs `mochi reload`.

## Testing

```sh
cargo test --workspace
```

This includes end-to-end tests that run the real `mochid` with a fake Quickshell and talk to it as the UI and the CLI. `nix flake check` runs the same suite in the package build.

To watch the island go through the arbiter's rules on your screen and record it to `/tmp/mochi-recordings/demo.mp4` (`MOCHI_RECORD_OUTPUT` changes the path; recordings stay out of the repository):

```sh
cargo test -p mochid --test record -- --ignored --nocapture
```

## Modules

| Module | What it does |
|---|---|
| `idle` | The clock shown when nothing else is. Clicking it toggles the hub; `click = ["module", "action", ...]` under `[module.idle]` runs something else, `[]` nothing. |
| `osd` | Shows volume, mute, output device switches, microphone mute, Caps Lock and Num Lock as they change, from any source. Needs pipewire-pulse or PulseAudio. |
| `workspaces` | Shows a monitor's workspaces when you switch, when focus moves to it, when one asks for attention, or when they're created or removed. Click a dot to switch. Needs a compositor with `ext-workspace-v1`. |
| `media` | Shows what's playing in any MPRIS player. A new track opens the island with the cover, progress and controls, then the music shrinks into a round bubble next to it: the cover with a progress ring. Click the bubble to bring the player back, use the buttons or click the bar to seek. With several players, arrows next to its name switch between them. `mochi ipc media play-pause`, `next`, `previous` and `seek <seconds>` do the same from a keybind. |
| `audio` | A volume mixer, as the hub's Sound page or on the island with `mochi ipc audio toggle`: the output and input with a choice of device, and a volume and mute for each app playing sound. `volume`, `mute`, `output` and `input` actions for keybinds. Needs pipewire-pulse or PulseAudio. |
| `notifications` | The notification daemon. Popups show a large app icon or picture, the app, the summary and the body; click one for the whole text and the app's buttons. A burst from one app shows only the latest (`same_app = "stack"` shows each in turn). Critical ones stay until closed. Missed ones go to a history behind a bell bubble next to the island. `mochi ipc notifications dnd toggle` turns do not disturb on and off, and `history`, `clear`, `dismiss` and `invoke` do the rest. If another notification daemon is running, Mochi waits and takes over when it stops. |
| `launcher` | `mochi ipc launcher toggle` (bind it to a key) grows the island into a search over your apps: most used first, fuzzy search as you type, app actions like "New Private Window" when you search for them. Enter or a click starts the app through `uwsm app` or `systemd-run`; Escape or a click elsewhere closes it. |
| `hub` | `mochi ipc hub toggle` grows the island into a wide panel: a home screen of cards, and pages in a navbar at the bottom. Other modules provide them: media a Now Playing card, audio a Sound page, notifications a card and a history page, the hub itself the date and time. |
| `power` | A hub page with lock, log out, suspend, hibernate, reboot, reboot to firmware and shut down, showing only what logind allows; ending the session takes a second click. Power profiles when power-profiles-daemon runs. `mochi ipc power <action>` does the same without asking. |
| `capture` | Screenshots and recordings: `mochi ipc capture screenshot` freezes the screens so you drag a region right away, or switch to a window or a screen on the island, then shows the capture with copy, edit and delete buttons. `record` does the same through gpu-screen-recorder, with a red dot while it records. |
| `share` | The screen-share picker for xdg-desktop-portal-hyprland: screens and windows with live pictures, or a region, on the island, and a bubble while something shares the screen. Click the bubble to share something else without the app asking again. The home-manager module sets the portal up. |
| `clipboard` | A clipboard history: `mochi ipc clipboard toggle` (bind it to SUPER+V) searches what you copied, text and images, and pastes the entry you pick. It stays in memory until you log out, or encrypted on disk with a key from the Secret Service. Copies password managers mark as secret are skipped. |
| `tray` | Apps' tray icons, like Discord, Steam or nm-applet: a tray bubble opens a drawer with every app on the island, a right click shows the app's menu in Mochi's style, and apps in `pinned` get a bubble of their own. Mochi serves the StatusNotifierWatcher, or shows another tray's icons when one runs. |
| `network` | Wi-Fi, Ethernet, VPNs and airplane mode from NetworkManager: a bubble with the connection, a Network page to join networks (asking for passwords on the island), a home card, and notices when you connect or disconnect. |
| `bluetooth` | Bluetooth from BlueZ: a bubble with the connected device's battery, a page to connect, pair and forget devices, a home tile, and pairing questions on the island. |
| `battery` | A laptop's battery from UPower: a short notice as it drops past 80, 50, 20 and 10%, a warning bubble at or under 50%, red at 10%, notices on plugging in or out, and a hub card. Every level is a setting. |
| `performance` | CPU, memory and GPU use and temperatures: a hub page with graphs and the busiest processes, a notice when a reading stays high naming the busiest process, and a red bubble while one stays critical. Every level is a setting. |
| `demo` | Test views and `mochi ipc demo` actions for trying the island. |

## Configuration

The first start writes `~/.config/mochi/config.toml` and `theme.toml`, with every option commented and set to its default; Mochi never overwrites them. Remove the `# ` in front of an option to change it.

```sh
mochi config path     # where the files are
mochi config check    # check them, with the errors mochid would give
mochi reload          # apply changes without a restart
```

`mochi reload` starts modules you added, stops the ones you removed and restarts the ones whose settings changed. A file with an error changes nothing.

## Plugins

Plugins add modules, with the same powers as the builtin ones. List them in `~/.config/mochi/plugins.toml`, install them, and enable them in `modules`:

```toml
[plugins.pomodoro]
source = "git:github.com/User/mochi-pomodoro:main"   # or git-release:…:v1, or path:~/code/x
```

```sh
mochi plugins install    # shows what each one runs and asks first; pins them in plugins.lock
mochi plugins update     # moves the pins
mochi plugins list
```

A plugin is a manifest, `mochi-plugin.toml`, QML views, and usually a backend: any program that speaks the plugin protocol over a socket mochid gives it. `crates/mochi-sdk` writes backends in Rust, and `examples/plugins` has two to start from, a pomodoro timer and the weather. `examples/python/hello` is one in Python, with a small SDK of its own. The documentation site covers installing plugins, writing them, the manifest, the Rust SDK and making an SDK for another language, and has the SDK's API reference.

## Documentation

The documentation site lives in `docs/book` and uses mdBook. `mdbook serve docs/book` previews it from the dev shell, and `nix build .#docs` builds it. Its configuration pages include each module's `settings.toml`, the same files the generated config is made of, so the two always agree. `.github/workflows/docs.yml` publishes it to GitHub Pages once the repository is on GitHub.

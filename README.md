# Mochi

A desktop shell built around a central island, like a dynamic island. `mochid`, a Rust daemon, owns state and system integration and supervises a Quickshell UI. `mochi` is its command-line client.

Early work in progress: the island, the idle clock and a demo module work today. `TODO.md` has the plan, `docs/protocol.md` the daemon's protocol, `docs/views.md` how to write views with the built-in controls and `docs/spike.md` the results of the first prototype.

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

The flake's `mochi` package has both binaries, the systemd unit, and the pinned Quickshell `mochid` expects. On NixOS:

```nix
# flake inputs
mochi.url = "path:/path/to/mochi-shell";

# configuration
environment.systemPackages = [ inputs.mochi.packages.${pkgs.system}.default ];
systemd.packages = [ inputs.mochi.packages.${pkgs.system}.default ];
```

Then enable it for your user:

```sh
systemctl --user enable --now mochid
```

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
| `media` | Shows what's playing in any MPRIS player. A new track opens the island with the cover, progress and controls, then the music shrinks into a round bubble next to it: the cover with a progress ring. Click the bubble to bring the player back, use the buttons or click the bar to seek. `mochi ipc media play-pause`, `next`, `previous` and `seek <seconds>` do the same from a keybind. |
| `notifications` | The notification daemon. Popups show a large app icon or picture, the app, the summary and the body; click one for the whole text and the app's buttons. A burst from one app shows only the latest (`same_app = "stack"` shows each in turn). Critical ones stay until closed. Missed ones go to a history behind a bell bubble next to the island. `mochi ipc notifications dnd toggle` turns do not disturb on and off, and `history`, `clear`, `dismiss` and `invoke` do the rest. If another notification daemon is running, Mochi waits and takes over when it stops. |
| `launcher` | `mochi ipc launcher toggle` (bind it to a key) grows the island into a search over your apps: most used first, fuzzy search as you type, app actions like "New Private Window" when you search for them. Enter or a click starts the app through `uwsm app` or `systemd-run`; Escape or a click elsewhere closes it. |
| `hub` | `mochi ipc hub toggle` grows the island into a wide panel: a home screen of cards, and pages in a navbar at the bottom. Other modules provide them: media a Now Playing card, notifications a card and a history page, the hub itself the date and time. |
| `power` | A hub page with lock, log out, suspend, hibernate, reboot, reboot to firmware and shut down, showing only what logind allows; ending the session takes a second click. Power profiles when power-profiles-daemon runs. `mochi ipc power <action>` does the same without asking. |
| `demo` | Test views and `mochi ipc demo` actions for trying the island. |

## Configuration

Both files are optional and live in `~/.config/mochi/`. Without a `config.toml`, only `idle` runs.

```toml
# config.toml
modules = ["idle", "osd", "workspaces", "media"]

[module.idle]
format = "HH:mm:ss"

[module.osd]
timeout_ms = 1500
microphone = false   # volume, device, microphone and locks can each be turned off

[module.media]
ignore = ["firefox"]   # players never shown

[bubbles.media]        # move any module's bubbles
area = "left"          # left, center-left, center, center-right or right
group = "status"       # bubbles with the same group share a pill
wide = true            # text pills instead of small round bubbles
```

```toml
# theme.toml: any token left out keeps its default
[colors]
accent = "#30d158"      # also background, surface, raised, highlight, foreground,
                        # muted, on_accent, danger, success

[text]
family = "Inter"        # empty keeps the system font"

[layout]
mode = "notch"     # "island" floats; "notch" attaches to the edge with curved corners
anchor = "top"     # or "bottom"
island = "center"  # left, center-left, center, center-right or right
margin = 6         # gap to the edges in island mode
spacing = 8        # gap between the island and bubbles

[layout.notch]
ear_radius = 10    # size of the curves that flare into the edge

[motion]
damping = 0.5
```

`mochi reload` applies `theme.toml` changes without a restart.

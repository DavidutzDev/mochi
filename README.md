# Mochi

A desktop shell built around a central island, like a dynamic island. `mochid`, a Rust daemon, owns state and system integration and supervises a Quickshell UI. `mochi` is its command-line client.

Early work in progress: the island, the idle clock and a demo module work today. `TODO.md` has the plan, `docs/protocol.md` the daemon's protocol and `docs/spike.md` the results of the first prototype.

## Running it

Everything runs from the dev shell (`nix develop`, or `direnv allow` once).

```sh
cargo run -p mochid -- --dev --modules idle,demo
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

## Configuration

Both files are optional and live in `~/.config/mochi/`.

```toml
# config.toml
modules = ["idle", "demo"]

[module.idle]
format = "HH:mm:ss"
```

```toml
# theme.toml: any token left out keeps its default
[colors]
accent = "#30d158"

[motion]
damping = 0.5
```

`mochi reload` applies `theme.toml` changes without a restart.

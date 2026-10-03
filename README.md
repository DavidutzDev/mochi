# Mochi

A desktop shell built around a central island, like a dynamic island. `mochid`, a Rust daemon, owns state and system integration and supervises a Quickshell UI. `mochi` is its command-line client.

Early work in progress: the island, the idle clock and a demo module run today. `TODO.md` has the plan, `docs/protocol.md` the daemon's protocol and `docs/spike.md` the results of the first prototype.

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

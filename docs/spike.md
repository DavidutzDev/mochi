# Spike findings

The spike checked the parts of the design that everything else depends on. It was throwaway code and has been removed. Commit `c441f31` still has it: run `git checkout c441f31` to try it again, and the commands below work from there.

Tested on 2026-10-03 with Quickshell 0.3.1 (revision `41651d7`), Hyprland 0.56.2 and two 1920x1080 monitors.

## Running it

```sh
nix develop
cd spike
cargo run -- run            # embedded QML, copied to $XDG_RUNTIME_DIR/mochi-spike/shell
cargo run -- run --dev      # symlinks to spike/qml, edits hot-reload
```

From another terminal:

```sh
cargo run -q -- ipc demo show Small
cargo run -q -- ipc demo show Wide "Some song - Some artist"
cargo run -q -- ipc demo show Card "Any text. A longer text makes a taller card."
cargo run -q -- ipc demo show Big
cargo run -q -- ipc idle show
```

To watch the whole sequence without typing commands, run the recording test. It starts the spike, shows every view, waits for the timeout, and saves a video of it:

```sh
cargo test --test demo -- --ignored --nocapture
```

It records the top center of the focused monitor, using `hyprctl` to find it. On other compositors, set `MOCHI_RECORD_GEOMETRY="x,y 640x640"`. Plain `cargo test` skips it, because it draws on the screen.

Clicking the idle island opens the card, and clicking anything else returns to idle. Demo activities time out after 4 seconds, and hovering pauses the timeout.

Blur needs no compositor configuration. The island requests it through the `ext-background-effect-v1` protocol.

## Results

| Question | Result |
|---|---|
| Island sizes itself from the view's implicit size | Works, from the 34px idle pill up to a 520x520 view. Views never declare a size to the island. |
| Shape goes from pill to rounded rectangle | Works with `radius: min(height / 2, maxRadius)`, without per-view configuration. |
| Morph between views | Works. Spring on width and height, old view fades out, new view fades in after 90ms. Settles in about 200 to 300ms. |
| Clicks outside the island reach windows | Works. With `mask: Region { item: island }` the compositor delivers no pointer events to the transparent area. Checked against a control run with the mask removed. |
| Exclusive zone stays at the idle size | Works. Hyprland reserves 40px for the island (plus 45px for Waybar) whether the island is idle or 520px tall. |
| Blur on the island only | Works through `ext-background-effect-v1` (Quickshell's `BackgroundEffect.blurRegion`), with no compositor rule. The blur region follows the island while it resizes. A Hyprland layer rule with `ignore_alpha` also works, but needs per-compositor config. |
| Rust spawns Quickshell on a generated directory | Works with `quickshell --path <dir>`. The socket path reaches QML through `MOCHI_SOCKET` and `Quickshell.env()`. |
| Socket round trip | Works. `Quickshell.Io.Socket` with `SplitParser` reads JSON lines, and writes go back with `write()` and `flush()`. |
| Reconnect after a UI restart | Works. The UI reconnects within about 100ms and the daemon re-sends the current activity. |
| Quickshell crash | Restarted 250ms after a SIGKILL, with backoff doubling on repeated exits. |
| Hung Quickshell | A stopped instance that never said hello was killed after the 5s handshake timeout and replaced. |
| Daemon killed with SIGKILL | Quickshell exits with it, through `PR_SET_PDEATHSIG`. |
| Stale socket file after a daemon crash | Removed on the next start. A second daemon is refused while the first is running. |
| Assets written only when changed | Works. A second start writes 0 files, so a daemon restart doesn't reload the UI. |
| Hot reload in dev mode | Works for core files and module views, through symlinks, with both in-place and rename-style saves. Needs the two fixes below. |

## Things that would have broken without a fix

**Quickshell only watches files it can reach through imports.** Its scanner follows `import` lines from `shell.qml`. Module views are loaded by URL at runtime, so nothing imports them and edits wouldn't hot-reload. The asset writer now generates `Modules.qml` at the shell root, which imports every enabled module directory. It is never instantiated. The scanner reads it and watches the module files.

**Views must load through `root:` URLs, not file paths.** `Quickshell.shellPath()` returns a plain path. A view loaded from a plain path sits outside Quickshell's config tree, which Quickshell's source says breaks singletons. In the spike it also meant edits to the view on screen didn't hot-reload. Loading `root:/modules/<id>/<View>.qml` fixed both.

**A hung UI ignores SIGTERM.** The handshake timeout originally sent SIGTERM, which a stopped process never handles. The daemon now sends SIGKILL, because it only kills Quickshell when it has stopped responding.

**The daemon must claim the socket before writing assets.** A second daemon started with a different module list would otherwise rewrite the running daemon's shell directory before failing.

## Notes for the real implementation

- `PR_SET_PDEATHSIG` fires when the spawning *thread* exits, not the process. Spawn Quickshell from a thread that lives as long as the daemon. With tokio, that means a dedicated thread, not a task on a worker thread.
- Quickshell prints `console.warn` from QML to stderr, so the supervisor's log forwarding catches QML warnings. `console.log` is filtered out by default.
- The island is centered in its surface, and during a morph the new view is revealed from the center. Anchoring views to the top edge may look closer to a Dynamic Island. Try both when the real island is built.
- The island sits below Waybar here, because Waybar claims the top 45px first. If Mochi replaces Waybar this goes away. Otherwise the layer order is something to decide.
- Compositors without `ext-background-effect-v1` show the island without blur. Hyprland 0.56 has it. Support in niri, mango and others is not checked yet.
- Hyprland 0.56 configures in Lua. `hyprctl dispatch` now takes Lua dispatchers, for example `hyprctl dispatch 'hl.dsp.cursor.move({x=100, y=100})'`, and `hyprctl eval` runs Lua.
- Visual checks can be automated: `grim -g "x,y wxh"` captures the island, and `wlrctl pointer move` / `wlrctl pointer click` generate real pointer events. A cursor warp alone sends no events to clients. Both tools are in the dev shell.

# How it works

Mochi is two programs that talk over a socket: `mochid`, a daemon written in Rust, and a user interface written in QML that runs in [Quickshell](https://quickshell.org). The daemon decides and the interface draws.

```
   D-Bus, PipeWire, NetworkManager, BlueZ, UPower, logind, the compositor…
                                   │
                                mochid ─── modules, the island's rules, bubbles
                                   │
            ┌──────────────────────┼──────────────────────┐
     Quickshell UI          mochi (the CLI)         plugin backends
```

## The daemon decides, the interface draws

`mochid` holds every piece of state and does all the work with the system: it listens to notifications on D-Bus, reads the volume from PipeWire, asks NetworkManager for Wi-Fi networks, watches the compositor's workspaces. It decides what the island shows and when, and tells the interface.

The interface only draws. It gets the state and the theme from the daemon and sends back what the user did: a click, a key, a drag. It holds no logic of its own beyond layout and animation.

This split is the reason Mochi exists instead of a shell written in Quickshell alone:

- **The work happens in compiled Rust,** outside the thread that draws. Stitching screenshots together, polling sensors and talking to D-Bus never make an animation stutter, and the QML stays small.
- **The logic is tested.** The rules for what shows, every module's behavior and the configuration run in the test suite, including end-to-end tests that drive the real daemon as the interface and the CLI would.
- **A crash of the interface loses nothing.** `mochid` starts Quickshell, restarts it when it exits, and sends it everything again: the state, the theme, what each island shows. Notifications, the clipboard history and a recording in progress are untouched.
- **Everything is reachable from outside.** Anything the interface can do, `mochi ipc` can do from a keybind or a script.

## Modules

Every feature is a module: the clock, notifications, the launcher, Bluetooth. A module has a Rust part that runs inside `mochid` and QML views that the interface draws. `modules` in `config.toml` lists which run, and a module that's off costs nothing.

Modules offer things to each other through contributions: a card or a page for the hub, results for the launcher, a widget for the desktop, a step for the tour. The hub, for instance, has little of its own besides the date and time. The modules that run fill it.

At start, `mochid` writes the views of the modules that run into `$XDG_RUNTIME_DIR/mochi/shell/` and points Quickshell at it, so what the interface loads always matches what runs.

## The island

The island is the shape at the top of each screen. Modules don't draw on it directly: they submit *activities*, like "a notification from Firefox" or "the volume changed", and a set of rules picks what shows:

- One activity shows at a time. One with a higher priority interrupts a lower one, which comes back when it ends.
- Others wait their turn, by priority, then by arrival.
- An activity with a key replaces the module's earlier one with the same key: dragging the volume updates one notice instead of queueing many.
- Timers pause while the pointer is over the island, and while it's expanded.

Each screen has its own island and decides by itself, so a panel open on one screen doesn't hold back a notice on another. [The island](configuration.md#the-island) has its settings.

## Bubbles

Bubbles are the small round items next to the island: the music playing, a recording's red dot, missed notifications. Modules ask for them, and the edge has five areas they fill, from left to right. A full area hides the rest behind a "+N", or stacks them into one. [Bubbles](bubbles.md) has the details.

## Configuration

`config.toml` says which modules run and how, and `theme.toml` how everything looks. Mochi writes both, commented, on the first start, and never changes them. `mochi reload` applies them without a restart, starting, stopping and restarting only the modules that changed.

The settings panel doesn't write those files either: it keeps its changes in `changes.toml` next to them, laid over them. That's what lets it work when home-manager owns your config, and why Copy in the panel hands the changes back as Nix or TOML for your own files. [Configuration](configuration.md) has the rest.

## Plugins

A plugin is a module that isn't built into Mochi, with the same powers. It's a manifest, `mochi-plugin.toml`, QML views, and usually a backend: any program, in any language, that `mochid` starts and talks to over a socket, with the same messages the builtin modules use. A plugin of views only can also replace a builtin view.

`mochi plugins install` fetches and builds them, and shows what each one will run before asking. [Plugins](plugins.md) covers using them, and [Writing plugins](writing-plugins.md) making one.

## Bento

[Bento](bento.md) shares what you made: a theme, a plugin, or a whole setup, from a reviewed registry or anyone's repository. It's off until you turn it on, and each setup you install is one you can switch to and back from.

## Compositors

Mochi reads workspaces, windows and screens through standard Wayland protocols, like `ext-workspace-v1` and `wlr-foreign-toplevel-management`, so it works on any compositor that has them: Hyprland, niri and Sway. Where a compositor offers more through its own IPC, Mochi uses it: Hyprland, for example, tells it what's being shared on screen. `mochi doctor` says what yours offers.

## Talking to it

Everything goes through the socket in `$XDG_RUNTIME_DIR/mochi/mochi.sock`, one JSON message per line. The interface, the `mochi` command and plugin backends are its three kinds of client. [Protocol](protocol.md) describes the messages.

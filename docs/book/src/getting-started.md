# Getting started

## The first start

The first time `mochid` starts, it writes two files to `~/.config/mochi/`:

- `config.toml`: which modules run, and their settings.
- `theme.toml`: colors, sizes and motion.

Every option is in them, commented, with its default. They turn on the whole shell: the clock, the on-screen display, workspaces, media, notifications, the launcher, the hub, the power page and screenshots. Mochi never overwrites these files.

```sh
mochi config path     # where the files are
mochi config check    # check them, without a running daemon
mochi reload          # apply your changes, without a restart
```

## Keys

Mochi doesn't grab keys itself. Bind these in your compositor:

| Action | Command |
|---|---|
| Open or close the launcher | `mochi ipc launcher toggle` |
| Open or close the hub | `mochi ipc hub toggle` |
| Play or pause the music | `mochi ipc media play-pause` |
| Do not disturb | `mochi ipc notifications dnd toggle` |
| Lock the screen | `mochi ipc power lock` |
| Take a screenshot | `mochi ipc capture screenshot` |
| Start or stop recording | `mochi ipc capture record` |

`mochi ipc` lists every action of every module, and `mochi ipc <module>` one module's.

## Using the island

- A left click expands what the island shows, or collapses it; clicking the clock opens the hub.
- A right click closes it.
- Hovering keeps it from timing out.
- Clicking a bubble opens what it stands for: the music bubble shows the player, the bell shows missed notifications.

## Another notification daemon

If swaync, mako or dunst is running, Mochi waits behind it and takes over notifications when it stops. To switch, stop the other one, for example with `systemctl --user disable --now swaync`.

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

In Hyprland's Lua config:

```lua
hl.bind("SUPER + space", hl.dsp.exec_cmd("mochi ipc launcher toggle"))
hl.bind("SUPER + C", hl.dsp.exec_cmd("mochi ipc hub toggle"))
hl.bind("SUPER + SHIFT + N", hl.dsp.exec_cmd("mochi ipc notifications dnd toggle"))
hl.bind("Print", hl.dsp.exec_cmd("mochi ipc capture screenshot"))
hl.bind("ALT + Print", hl.dsp.exec_cmd("mochi ipc capture record"))
-- `locked` keeps media keys working on the lock screen.
hl.bind("XF86AudioPlay", hl.dsp.exec_cmd("mochi ipc media play-pause"), { locked = true })
hl.bind("XF86AudioNext", hl.dsp.exec_cmd("mochi ipc media next"), { locked = true })
hl.bind("XF86AudioPrev", hl.dsp.exec_cmd("mochi ipc media previous"), { locked = true })
```

In `hyprland.conf`:

```ini
bind = SUPER, space, exec, mochi ipc launcher toggle
bind = SUPER, C, exec, mochi ipc hub toggle
bind = , Print, exec, mochi ipc capture screenshot
bindl = , XF86AudioPlay, exec, mochi ipc media play-pause
```

In Sway:

```
bindsym $mod+space exec mochi ipc launcher toggle
bindsym $mod+c exec mochi ipc hub toggle
bindsym Print exec mochi ipc capture screenshot
bindsym --locked XF86AudioPlay exec mochi ipc media play-pause
```

In niri:

```kdl
binds {
    Mod+Space { spawn "mochi" "ipc" "launcher" "toggle"; }
    Mod+C { spawn "mochi" "ipc" "hub" "toggle"; }
    Print { spawn "mochi" "ipc" "capture" "screenshot"; }
    XF86AudioPlay allow-when-locked=true { spawn "mochi" "ipc" "media" "play-pause"; }
}
```

## The command line

`mochi completions <shell>` prints completions for bash, zsh, fish, elvish or PowerShell; the Nix and Arch packages install them for bash, zsh and fish. `--json` makes `mochi status`, `mochi ipc list`, `mochi ipc <module>` and `mochi plugins list` print JSON, for scripts:

```sh
mochi status --json | jq -r '.modules[]'
```

## Using the island

- A left click expands what the island shows, or collapses it; clicking the clock opens the hub.
- A right click closes it.
- Hovering keeps it from timing out.
- Clicking a bubble opens what it stands for: the music bubble shows the player, the bell shows missed notifications.

## Another notification daemon

If swaync, mako or dunst is running, Mochi waits behind it and takes over notifications when it stops. To switch, stop the other one, for example with `systemctl --user disable --now swaync`.

# Lock

Mochi's own lock screen, in place of hyprlock or swaylock. It's off until you add `"lock"` to `modules`, and it needs a PAM service, below.

A small program of its own, `mochi-lock`, takes the compositor's session lock (`ext-session-lock-v1`, which Hyprland, niri and Sway have) and checks your password with PAM. From the moment the compositor confirms the lock, it shows nothing but the lock screen on every monitor, a monitor plugged in while locked too, and gives the keyboard to nothing else. Only `mochi-lock` can unlock, and only once PAM accepts the password: there is no command that unlocks, and logind's unlock request is ignored.

By default the lock screen shows the time, big, with the date and the weather under it, and on the right what's playing, with play, pause and skip, and the field, with a dot for each character typed. The battery's level sits in a corner. Behind it all is your wallpaper, blurred and dimmed. Escape or Ctrl+U clears the field, Enter checks it. After a wrong password it says so, or says what PAM said, like an account locked after too many tries; from the third wrong one in a row, Enter waits 2 seconds, then 4, 8 and so on, up to a minute, on top of PAM's own delays. It says when Caps Lock is on. Its colors and fonts are the theme's.

`background` in `[module.lock]` picks what's behind it: the wallpaper that awww, swww or hyprpaper shows, or Hyprland's own when none does; an image; a screenshot of each monitor taken just before locking, open windows and all; or the theme's color. The images are blurred. A screenshot is taken only for a lock you start, never for a lock screen started again after a crash, which would only show the lock, and it's deleted once unlocked. Wayland has no way to capture only what's under the windows, so a screenshot shows them, blurred. `mochi ipc lock edit` arranges [what's on it](#arranging-it). Weather shows once the [weather](weather.md) module runs with a place, and what's playing once the [media](media.md) module runs.

## Arranging it

`mochi ipc lock edit`, or Arrange the lock screen on the Lock page of the settings, arranges the lock screen the way `mochi ipc widgets edit` arranges the desktop: over its background, with the same grid, dragging, resizing, settings and layers, and the island's notice opening the drawer. The drawer has every widget, and the password field. Run it again, Escape or Done to stop.

On the lock screen, widgets only show: they never get a key, so a note can be read but not typed into, and their buttons do nothing but play, pause and skip what's playing. What they show is yours to decide: a note or a to-do list on the lock screen can be read by anyone at the screen. Plugins' widgets go there only with `lock_plugins = true` in `[module.widgets]`. A widget can keep itself off the lock screen with `places = ["desktop"]` in its offer.

The password field is always there: it can't be removed, only moved and resized, and a layout without one gets it back.

The layout is kept in `lock-widgets.toml`, next to `widgets.toml`, in the same form; the default one has the clock on the left, the weather under it, what's playing over the field on the right and the battery in a corner. A widget whose `output` is empty, as in the default, shows on every monitor.

## How it draws

`mochi-lock` holds the lock and the keyboard. The lock screen itself is Quickshell, like the rest of Mochi, but it draws on a small display of `mochi-lock`'s own, not on your compositor: `mochi-lock` copies what it draws onto the lock's surfaces. That display has no keyboard, so the lock screen's views never see a key: they show as many dots as `mochi-lock` says, and nothing else about the password reaches them. They can't unlock, and the commands they may ask for are play, pause and skip, which `mochi-lock` and mochid both check. Only Quickshell's own process may connect to that display, and it runs without the variables that would lead it to your compositor or its IPC.

Until the lock screen draws, or when it stops drawing a change within a moment and a half, or crashes, `mochi-lock` draws a plain face of its own: the time, the date and the field. It starts the lock screen again after a crash. Either way the password works the same.

## When it locks

- `mochi ipc lock lock`, bound to a key or run from a script.
- When logind asks: `loginctl lock-session`, the [power](power.md) module's Lock, or an idle daemon. hypridle's usual `lock_cmd`, `loginctl lock-session`, works as is.
- Before the computer sleeps: Mochi holds the sleep back until the compositor confirms the lock, 2 seconds at most, so waking up never shows the desktop.

The power module's `lock` setting, when set, runs that command instead, like `["hyprlock"]`.

## What it forgets

Once the screen is locked, the [auth](auth.md) module, when it runs, forgets sudo's password, the SSH keys in the agent, polkit's admin actions and the keyring, or keeps them as long as its settings say.

## Setting it up

`mochi-lock` checks the password with the PAM service `mochi`, a file in `/etc/pam.d`. Without it, Mochi refuses to lock, since nothing could unlock the screen, and says so on the island.

- NixOS: the NixOS module adds it, unless `programs.mochi.lock.enable = false`. Without the module, add `security.pam.services.mochi = {};` to your system's configuration.
- Arch: the packages install `/etc/pam.d/mochi`.
- The release archive: `sudo install -m644 ~/.local/share/mochi/pam.d/mochi /etc/pam.d/mochi`.

Or point `pam_service` at a service you have, like `hyprlock`. On Hyprland, set `misc:allow_session_lock_restore = true`, so a new lock screen can take over from one that crashed. `mochi doctor` checks all of this.

```toml
{{#include ../../../../modules/lock/settings.toml}}
```

| Action | What it does |
|---|---|
| `lock` | Locks the screen |
| `status` | Says `locked`, `locking` or `unlocked` |
| `edit` | Arranges the lock screen's widgets, or stops |

## When something fails

If `mochi-lock` crashes or is killed, the compositor keeps the screen locked: it shows nothing of your desktop, on Hyprland and Sway a plain color. Mochi starts the lock screen again within a moment. A lock outlives Mochi itself too: a restarted mochid finds it and watches it, or starts a new one if it's gone.

If nothing comes back, switch to another TTY with Ctrl+Alt+F2, log in, and run `WAYLAND_DISPLAY=wayland-1 mochi-lock`, with your session's display, to take the lock over and unlock it with your password.

## What it defends against

Someone at your keyboard while you're away: typing, clicking, plugging in a monitor, closing the lid, or waiting for something to crash. Not a program already running as you, which could read your keystrokes anyway, and not someone logging in on another TTY with their own account.

What's typed stays in `mochi-lock`: it's never written to a log, never sent to mochid, and wiped from memory after each try. An empty password never unlocks, whatever the PAM service would say.

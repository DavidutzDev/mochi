# Lock

Mochi's own lock screen, in place of hyprlock or swaylock. It's off until you add `"lock"` to `modules`, and it needs a PAM service, below.

A small program of its own, `mochi-lock`, takes the compositor's session lock (`ext-session-lock-v1`, which Hyprland, niri and Sway have) and checks your password with PAM. From the moment the compositor confirms the lock, it shows nothing but the lock screen on every monitor, a monitor plugged in while locked too, and gives the keyboard to nothing else. Only `mochi-lock` can unlock, and only once PAM accepts the password: there is no command that unlocks, and logind's unlock request is ignored.

The lock screen shows the time and the date, and a field with a dot for each character typed. Escape or Ctrl+U clears it, Enter checks it. After a wrong password it says so, or says what PAM said, like an account locked after too many tries; from the third wrong one in a row, Enter waits 2 seconds, then 4, 8 and so on, up to a minute, on top of PAM's own delays. It says when Caps Lock is on. Its colors are the theme's.

## When it locks

- `mochi ipc lock lock`, bound to a key or run from a script.
- When logind asks: `loginctl lock-session`, the [power](power.md) module's Lock, or an idle daemon. hypridle's usual `lock_cmd`, `loginctl lock-session`, works as is.
- Before the computer sleeps: Mochi holds the sleep back until the compositor confirms the lock, 2 seconds at most, so waking up never shows the desktop.

The power module's `lock` setting, when set, runs that command instead, like `["hyprlock"]`.

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

## When something fails

If `mochi-lock` crashes or is killed, the compositor keeps the screen locked: it shows nothing of your desktop, on Hyprland and Sway a plain color. Mochi starts the lock screen again within a moment. A lock outlives Mochi itself too: a restarted mochid finds it and watches it, or starts a new one if it's gone.

If nothing comes back, switch to another TTY with Ctrl+Alt+F2, log in, and run `WAYLAND_DISPLAY=wayland-1 mochi-lock`, with your session's display, to take the lock over and unlock it with your password.

## What it defends against

Someone at your keyboard while you're away: typing, clicking, plugging in a monitor, closing the lid, or waiting for something to crash. Not a program already running as you, which could read your keystrokes anyway, and not someone logging in on another TTY with their own account.

What's typed stays in `mochi-lock`: it's never written to a log, never sent to mochid, and wiped from memory after each try. An empty password never unlocks, whatever the PAM service would say.

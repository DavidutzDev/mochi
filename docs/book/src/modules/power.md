# Power

A page in the [hub](hub.md) with lock, log out, suspend, hibernate, reboot, reboot to firmware and shut down. Only what the machine allows shows. Logging out, rebooting and shutting down take a second click, so a stray click never ends your session. When power-profiles-daemon runs, the page also switches between Power saver, Balanced and Performance.

## Keep awake

The page's Keep awake switch, or `mochi ipc power awake`, keeps the screen on and the machine awake: no screen lock, no screen off and no automatic sleep. It works two ways at once. Mochi holds a logind inhibitor for idle and sleep, which hypridle and systemd's automatic suspend respect, and the island asks the compositor not to go idle through Wayland's idle inhibit protocol, which swayidle and Hyprland respect. A cup stays next to the island while it's on; a click on it turns it off. Closing the lid or picking Suspend still suspends.

```toml
{{#include ../../../../modules/power/settings.toml}}
```

| Action | What it does |
|---|---|
| `lock`, `logout`, `suspend`, `hibernate`, `reboot`, `firmware`, `shutdown` | Act at once, without asking |
| `profile <name>` | Switches the power profile |
| `awake [on\|off\|toggle]` | Keeps the machine awake, or stops; flips it without an argument |

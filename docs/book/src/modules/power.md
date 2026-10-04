# Power

A page in the [hub](hub.md) with lock, log out, suspend, hibernate, reboot, reboot to firmware and shut down. Only what the machine allows shows. Logging out, rebooting and shutting down take a second click, so a stray click never ends your session. When power-profiles-daemon runs, the page also switches between Power saver, Balanced and Performance.

```toml
{{#include ../../../../modules/power/settings.toml}}
```

| Action | What it does |
|---|---|
| `lock`, `logout`, `suspend`, `hibernate`, `reboot`, `firmware`, `shutdown` | Act at once, without asking |
| `profile <name>` | Switches the power profile |

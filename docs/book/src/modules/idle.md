# Idle

The clock the island shows when nothing else is happening. Clicking it opens the [hub](hub.md).

Resting the pointer on it can run another action, `hover`. For the workspace dots, to click or scroll through:

```toml
[module.idle]
hover = ["workspaces", "show"]
```

`hover` waits for the pointer to rest on the clock for `hover_delay_ms`, so passing over it does nothing and a quick click still runs `click`. Whatever it shows stays while the pointer is on the island, and goes about a second after the pointer leaves.

```toml
{{#include ../../../../modules/idle/settings.toml}}
```

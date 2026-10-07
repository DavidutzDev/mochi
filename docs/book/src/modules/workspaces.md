# Workspaces

Shows a monitor's workspaces as dots, with the active one stretched into a pill, when you switch, when focus moves to another monitor, when a workspace asks for attention, or when one is created or removed. Click a dot to switch to it, or scroll on the dots for the previous or next one.

`mochi ipc workspaces show` shows the dots on demand, for the monitor under the pointer or the one named, and keeps them while the pointer is on them. The [idle module](idle.md)'s `hover = ["workspaces", "show"]` shows them when the pointer rests on the clock.

It works on any compositor with the `ext-workspace-v1` protocol.

```toml
{{#include ../../../../modules/workspaces/settings.toml}}
```

| Action | What it does |
|---|---|
| `mochi ipc workspaces switch <monitor> <workspace>` | Switches a monitor to one of its workspaces |

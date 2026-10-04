# Workspaces

Shows a monitor's workspaces as dots, with the active one stretched into a pill, when you switch, when focus moves to another monitor, when a workspace asks for attention, or when one is created or removed. Click a dot to switch to it.

It works on any compositor with the `ext-workspace-v1` protocol.

```toml
{{#include ../../../../modules/workspaces/settings.toml}}
```

| Action | What it does |
|---|---|
| `mochi ipc workspaces switch <monitor> <workspace>` | Switches a monitor to one of its workspaces |

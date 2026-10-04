# Launcher

Grows the island into a search over your apps. Your most used apps come first; typing searches names, descriptions and keywords, and app actions like Firefox's "New Private Window" when you search for them. Enter or a click starts the app, Escape or a click elsewhere closes the launcher.

Apps start through `uwsm app` in a uwsm session, otherwise through `systemd-run`, so they never belong to `mochid`.

```toml
{{#include ../../../../modules/launcher/settings.toml}}
```

| Action | What it does |
|---|---|
| `toggle`, `open`, `close` | Shows or hides the launcher |
| `launch <id>` | Starts an app by desktop id, like `firefox.desktop` |

# Notifications

The notification daemon. Popups show the app's icon or picture, the app, the summary and the body; click one for the whole text and the app's buttons. A burst from one app shows only the latest. Critical notifications, like a low battery, stay until you close them.

Popups that time out go to the history, behind a bell bubble that counts them. Do not disturb sends everything but critical notifications straight to the history.

If another notification daemon runs, Mochi waits and takes over when it stops.

```toml
{{#include ../../../../modules/notifications/settings.toml}}
```

| Action | What it does |
|---|---|
| `history` | Lists the missed notifications on the island |
| `clear` | Removes every missed notification |
| `dnd on`, `dnd off`, `dnd toggle` | Do not disturb |
| `dismiss <id>` | Closes a notification |
| `invoke <id> <action>` | Runs one of a notification's buttons |

It offers the hub a card with the latest missed notifications, and a page with all of them.

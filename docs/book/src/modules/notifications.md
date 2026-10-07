# Notifications

The notification daemon. Popups show the app's icon or picture, the app, the summary and the body; click one for the whole text and the app's buttons, then Escape or a click outside closes it. A burst from one app shows only the latest. Critical notifications, like a low battery, stay until you close them.

Popups that time out go to the history, behind a bell bubble that counts them. Do not disturb sends everything but critical notifications straight to the history.

The body can have bold, italic, underline, line breaks and links (`body-markup`, `body-hyperlinks`). Mochi keeps `<b>`, `<i>`, `<u>`, `<br>` and `<a href>` to `http`, `https`, `mailto` and `file` URLs, drops every attribute but `href`, and shows any other tag as text. An image (`<img>`) shows its `alt` text instead, so a notification can't make Mochi load a picture from the web. A click on a link opens it with `xdg-open` and closes the notification, in the popup and in the history.

Apps that take a reply, like chat apps, send an `inline-reply` action, and Mochi advertises the `inline-reply` capability. That notification gets a Reply button; click it and type, then Enter sends the text back to the app as `NotificationReplied`, and Escape goes back to the buttons. A notification you clicked open never times out, so the text field waits for you. Notifications restored from before a restart have no Reply button, like their other buttons.

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
| `reply <id> <text>` | Answers a notification that takes a reply |
| `open <id> <url>` | Opens a link from a notification's body, and closes the notification |

It offers the hub a card with the latest missed notifications, and a page with all of them.

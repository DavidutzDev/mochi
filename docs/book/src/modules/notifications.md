# Notifications

The notification daemon. Popups show the app's icon or picture, the app, the summary and the body; click one for the whole text and the app's buttons, then Escape or a click outside closes it. A burst from one app shows only the latest. Critical notifications, like a low battery, stay until you close them.

Popups that time out go to the history, behind a bell bubble that counts them. Do not disturb sends everything but critical notifications straight to the history.

The body can have bold, italic, underline, line breaks and links (`body-markup`, `body-hyperlinks`). Mochi keeps `<b>`, `<i>`, `<u>`, `<s>`, `<br>` and `<a href>` to `http`, `https`, `mailto` and `file` URLs, drops every attribute but `href`, and shows any other tag as text. An image (`<img>`) shows its `alt` text instead, so a notification can't make Mochi load a picture from the web. A click on a link opens it with `xdg-open` and closes the notification, in the popup and in the history.

Some apps, like Discord, send the Markdown their messages were typed in, so Mochi reads that too: `**bold**`, `*italic*` or `_italic_`, `__underline__`, `~~strike~~`, `` `code` `` (shown without the backticks), `[text](https://…)` links, and `||spoilers||`, which show as "spoiler" so the popup doesn't give them away. Plain `https://…` links become links for every app. `_` only counts at a word's edge, so `snake_case` stays, and a mark needs text right inside it, so `2 * 3 * 4` stays too. `markdown = false` turns this off, if an app's bodies have stars or underscores that aren't meant as Markdown.

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

It offers the control center a card with the latest missed notifications, and a page with all of them.

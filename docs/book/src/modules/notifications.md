# Notifications

The notification daemon. Popups show the app's icon or picture, the app, the summary and the body; click one for the whole text and the app's buttons, then Escape or a click outside closes it. A burst from one app shows only the latest. Critical notifications, like a low battery, stay until you close them.

Popups that time out go to the history, behind a bell bubble that counts them. Do not disturb sends everything but critical notifications straight to the history.

The body can have bold, italic, underline, line breaks and links (`body-markup`, `body-hyperlinks`). Mochi keeps `<b>`, `<i>`, `<u>`, `<s>`, `<br>` and `<a href>` to `http`, `https`, `mailto` and `file` URLs, drops every attribute but `href`, and shows any other tag as text. An image (`<img>`) shows its `alt` text instead, so a notification can't make Mochi load a picture from the web. A click on a link opens it with `xdg-open` and closes the notification, in the popup and in the history.

Some apps, like Discord, send the Markdown their messages were typed in, so Mochi reads that too: `**bold**`, `*italic*` or `_italic_`, `__underline__`, `~~strike~~`, `` `code` `` (shown without the backticks), `[text](https://…)` links, and `||spoilers||`, which show as "spoiler" so the popup doesn't give them away. Plain `https://…` links become links for every app. `_` only counts at a word's edge, so `snake_case` stays, and a mark needs text right inside it, so `2 * 3 * 4` stays too. `markdown = false` turns this off, if an app's bodies have stars or underscores that aren't meant as Markdown.

Apps that take a reply, like chat apps, send an `inline-reply` action, and Mochi advertises the `inline-reply` capability. That notification gets a Reply button; click it and type, then Enter sends the text back to the app as `NotificationReplied`, and Escape goes back to the buttons. A notification you clicked open never times out, so the text field waits for you. Notifications restored from before a restart have no Reply button, like their other buttons.

If another notification daemon runs, Mochi waits and takes over when it stops.

## Sounds

Mochi advertises the `sound` capability and plays the sound an app asks for when its notification pops up: a file (`sound-file`) or a name from the sound theme (`sound-name`), like `message-new-instant`. A notification without either stays quiet, and so does one with `suppress-sound`. Files play through `pw-play`, or `paplay` without it. Names play through `canberra-gtk-play` when it's installed, which uses the theme your GTK settings name. Without it, Mochi looks for `sounds/<theme>/stereo/<name>.oga` (or `.ogg`, `.wav`) in `$XDG_DATA_HOME` and `$XDG_DATA_DIRS`, in `sound_theme` and then in `freedesktop`, and tries shorter names too: `message-new-instant`, then `message-new`, then `message`. Most distributions ship the `freedesktop` theme as `sound-theme-freedesktop`.

Only a new popup makes a sound. An app updating its notification, like a download's progress, doesn't play it again, and neither does one going straight to the history. Do not disturb silences every sound, critical ones included. A new sound stops the one before it, so a burst of messages doesn't play on top of itself, and no sound lasts more than 10 seconds.

Sounds are on by default, since apps only ask for one when they want it heard and GNOME and KDE play them too. `sounds = false` turns them off. `sound_command` replaces the players: it gets the file as its last argument, and names go through the theme lookup above first.

To try one:

```sh
notify-send --hint=string:sound-name:message-new-instant "Ada" "Lunch?"
notify-send --hint=string:sound-file:/usr/share/sounds/freedesktop/stereo/bell.oga "Bell" "Ding"
```

On NixOS the path starts with `/run/current-system/sw/share` instead.

## Settings and actions

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

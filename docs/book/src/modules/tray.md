# Tray

The icons apps put in a system tray, like Discord, Steam, Spotify or nm-applet. A tray bubble next to the island opens the drawer: every app's icon and name on the island. Click an app to do what a click on its icon does, usually showing its window. Right-click it for its menu, drawn in Mochi's style: entries with more open as a page of their own, and Escape or the back arrow goes up a level. A middle click does the app's second action.

Apps listed in `pinned` also get a bubble of their own, which takes the same clicks and the scroll wheel, the way some apps change the volume or the track. An app asking for attention makes its bubble, or the tray bubble, breathe in the accent color, and marks it in the drawer. Icons their app marks as passive, unimportant for now, stay in the drawer, dimmed.

```toml
{{#include ../../../../modules/tray/settings.toml}}
```

`[bubbles.tray]` moves the bubbles, like any module's.

## How it works

Apps register their icons with a StatusNotifierWatcher over D-Bus, and read the menu through DBusMenu. Mochi serves the watcher. Only one program can: if another tray already does, like Waybar's, Mochi shows the icons that tray collects instead. Apps that only speak the old XEmbed tray don't show; most apps speak both.

Icons come from the icon theme, from the app's own icon folder, or from the pixels the app sends, which Mochi writes as PNG files in its runtime directory.

| Action | What it does |
|---|---|
| `toggle`, `open`, `close` | Opens or closes the drawer |
| `list` | Prints each app's key, name and state |
| `activate <app>` | Does what a click on the app's icon does |
| `secondary <app>` | Does what a middle click does |
| `scroll <app> <delta> [vertical\|horizontal]` | Scrolls on the app's icon; 120 is one notch |
| `menu <app>` | Opens the app's menu on the island |
| `click <app> <entry>` | Clicks an entry of the open menu |

`<app>` is the key `list` prints, usually the app's id, or its name. The menu sends `submenu` itself, as a submenu opens, for apps that fill it only then.

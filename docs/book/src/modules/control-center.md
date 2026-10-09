# Control center

A panel that grows out of the island: a home screen of cards, and pages in a navbar at the bottom. It has no content of its own besides the date and time. Other modules provide the rest: media a Now Playing card, audio a volume card and a Sound page, brightness a slider per display, night light a tile, notifications a card and a page, power a page, clipboard a card and a page, capture a Captures page, network a card and a page, bluetooth a tile and a page, battery a card, performance a page. A module that isn't running provides nothing.

Clicking the clock opens it, and Escape or a click elsewhere closes it. A card whose module also has a page shows a chevron by its heading: clicking the heading, or the card beside its controls, opens that page.

| Action | What it does |
|---|---|
| `toggle`, `close` | Shows or hides the control center |
| `open [page]` | Opens it, on a page like `power/power` |
| `arrange <order> [hidden]` | Keeps the home's cards in an order and hides some, each a comma-separated list of cards like `network/status` |

It used to be called the hub. `hub` still works in `config.toml`, in `mochi ipc hub ...` and in plugin manifests, with a warning in the log, so older configs and keybinds keep working until you change them.

The home is a grid of three columns. The control center draws each card's frame, with the card's icon and title at the top, and places the cards in order, each in the first spot where it fits. A card is one or more columns wide and one or two rows tall. With the default modules, the first row has the Network toggles (two columns) and Bluetooth, the second Today, the latest missed notification and the battery, then Now playing takes two columns and two rows beside Clipboard and Colors. Plugin cards, like the weather, come after.

## Arranging the home

The pencil at the right of the navbar edits the home. Every card gets an outline: drag one onto another to put it there, and its minus takes it off. The cards taken off wait under the others, under "More cards"; a click puts one back at the end. Done keeps the arrangement, and Escape leaves the home as it was. The control center stays open while it saves.

The arrangement lives in two settings, `order` and `hidden`, written to `changes.toml` like the settings panel's changes, so Copy in the settings panel hands them to your Nix or TOML config. Cards listed in `order` come first, in that order; the others follow in their own. A module's card that appears later, from a module you turn on, joins at the end.

The panel takes the height of what it shows, up to `height` and the screen; a longer page scrolls, and the navbar stays in place.

```toml
{{#include ../../../../modules/control-center/settings.toml}}
```

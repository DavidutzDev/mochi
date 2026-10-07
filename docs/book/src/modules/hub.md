# Hub

A panel that grows out of the island: a home screen of cards, and pages in a navbar at the bottom. It has no content of its own besides the date and time. Other modules provide the rest: media a Now Playing card, audio a Sound page, notifications a card and a page, power a page, clipboard a card and a page, capture a Captures page, network a card and a page, bluetooth a tile and a page, battery a card, performance a page. A module that isn't running provides nothing.

Clicking the clock opens it, and Escape or a click elsewhere closes it. A card whose module also has a page shows a chevron by its heading: clicking the heading, or the card beside its controls, opens that page.

| Action | What it does |
|---|---|
| `toggle`, `close` | Shows or hides the hub |
| `open [page]` | Opens it, on a page like `power/power` |

The home is a grid of three columns. The hub draws each card's frame, with the card's icon and title at the top, and places the cards in order, each in the first spot where it fits. A card is one or more columns wide and one or two rows tall. With the default modules, the first row has the Network toggles (two columns) and Bluetooth, the second Today, the latest missed notification and the battery, then Now playing takes two columns and two rows beside Clipboard and Colors. Plugin cards, like the weather, come after.

The panel takes the height of what it shows, up to `height` and the screen; a longer page scrolls, and the navbar stays in place.

```toml
{{#include ../../../../modules/hub/settings.toml}}
```

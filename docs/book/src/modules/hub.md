# Hub

A panel that grows out of the island: a home screen of cards, and pages in a navbar at the bottom. It has no content of its own besides the date and time. Other modules provide the rest: media a Now Playing card, audio a Sound page, notifications a card and a page, power a page, clipboard a card and a page, capture a Captures page, network a card and a page, bluetooth a tile and a page, battery a card, performance a page. A module that isn't running provides nothing.

Clicking the clock opens it, and Escape or a click elsewhere closes it. It has no settings.

| Action | What it does |
|---|---|
| `toggle`, `close` | Shows or hides the hub |
| `open [page]` | Opens it, on a page like `power/power` |

Every page, the home included, has the same size, so the panel doesn't jump when you switch: a shorter page leaves room below, a longer one scrolls, and the navbar stays in place. It never grows past the screen.

```toml
{{#include ../../../../modules/hub/settings.toml}}
```

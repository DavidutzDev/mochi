# Control center

A panel that grows out of the island: a home screen of cards, and pages in a navbar at the bottom. It has no content of its own besides the date and time. Other modules provide the rest: media a Now Playing card, audio a volume card and a Sound page, brightness a slider per display and one for the keyboard, night light a tile, notifications a card and a page, power a page, clipboard a card and a page, capture a Captures page, network a card and a page, bluetooth a tile and a page, battery a card with the peripherals' batteries, performance a page, the focus timer a card, weather a card with the next hours, a smaller one and a page, the [clock](clock.md) a card with the next reminder or the stopwatch and a page with its tabs. A module that isn't running provides nothing.

Clicking the clock opens it, and Escape or a click elsewhere closes it. With the [clock module](clock.md) on, a click on the Today card opens the clock panel, with the calendar and the reminders. A card whose module also has a page shows a chevron by its heading: clicking the heading, or the card beside its controls, opens that page.

| Action | What it does |
|---|---|
| `toggle`, `close` | Shows or hides the control center |
| `open [page]` | Opens it, on a page like `power/power` |
| `edit` | Opens it arranging the home and the navbar, as the pencil does |
| `arrange <order> [hidden]` | Keeps the home's cards in an order and hides some, each a comma-separated list of cards like `network/status` |
| `arrange-pages <pages> [hidden]` | Keeps the navbar's pages in an order and hides some, the same way, with pages like `weather/page` |

It used to be called the hub. `hub` still works in `config.toml`, in `mochi ipc hub ...` and in plugin manifests, with a warning in the log, so older configs and keybinds keep working until you change them.

The home is a grid of three columns. The control center draws each card's frame, with the card's icon and title at the top, and places the cards in order, each in the first spot where it fits. A card is one or more columns wide and one or two rows tall. With the default modules, the first row has the Network toggles (two columns) and Bluetooth, the second Today, the latest missed notification and the battery, then Now playing takes two columns and two rows beside Clipboard and Colors, and the weather (two columns) comes last. Plugin cards come after. Some cards are spare, like the weather's one-column card: they wait under "More cards" until you put them on the home.

## Arranging the home and the navbar

The pencil at the right of the navbar, or `mochi ipc control-center edit`, arranges the home and the navbar. Every card gets an outline: drag one onto another to put it there, and its minus takes it off. The cards taken off wait under the others, under "More cards"; a click puts one back at the end. The navbar's pages get an outline and a minus too: drag a page along the navbar onto another to put it there, and its minus takes it out of the navbar. The pages taken out wait under "More pages"; a click puts one back at the end. Home stays first. Done keeps the arrangement, and Escape leaves everything as it was. The control center stays open while it saves.

The arrangement lives in four settings, `order` and `hidden` for the cards, and `pages` and `hidden_pages` for the navbar, written to `changes.toml` like the settings panel's changes, so Copy in the settings panel hands them to your Nix or TOML config. Done writes only what changed, so moving a card leaves the navbar's settings alone. Cards and pages listed come first, in that order; the others follow in their own. A module's card or page that appears later, from a module you turn on, joins at the end.

A page out of the navbar still opens: `mochi ipc control-center open weather/page` opens it, and so does its card's heading. While it's open, its tab shows in the navbar so you see where you are, and it leaves with the next page you pick. To be rid of a page altogether, turn its module off.

The panel takes the height of what it shows, up to `height` and the screen; a longer page scrolls, and the navbar stays in place.

```toml
{{#include ../../../../modules/control-center/settings.toml}}
```

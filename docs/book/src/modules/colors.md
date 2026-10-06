# Colors

A color picker. `mochi ipc colors pick`, bound to a key, freezes every screen and shows a magnifier next to the pointer: the pixels around it, enlarged, with the one under the pointer outlined and its hex below. A click picks that pixel; Escape or a right click cancels.

```lua
hl.bind("SUPER + SHIFT + P", hl.dsp.exec_cmd("mochi ipc colors pick"))
```

The picked color is copied in the `format` setting's format, kept in the history, and shown on the island for 8 seconds: a big swatch and a row for each format.

| Format | Example |
|---|---|
| HEX | `#1e1e2e` |
| RGB | `rgb(30, 30, 46)` |
| HSL | `hsl(240, 21%, 15%)` |
| OKLCH | `oklch(24% 0.03 284)` |

Clicking a row copies it in that format. With the [clipboard](clipboard.md) module enabled, copies go through it, so they're in its history too; without it, Mochi runs `wl-copy`.

The color is the exact pixel on the screen, also with fractional scaling. When the picker opens, Mochi copies every monitor through `wlr-screencopy`, which Hyprland and Sway support, and reads the pixels from that copy. The pointer's position is turned into the monitor's own pixels, so on a 1.5 scale the pixel picked is the one the pointer is over, not a blend of its neighbors.

## The history

The colors you pick are kept in `$XDG_STATE_HOME/mochi/colors.json`, newest first. Picking a color again moves it to the top, and only the newest `history` stay.

The hub has a Colors card with the last 8 colors as dots: hover one to see it in the default format, click it to copy it. Its Pick button closes the hub and opens the picker. The Colors page lists the whole history, each color with its four formats as buttons that copy them and a trash button that removes it, with "Pick a color" and "Clear" above. `mochi ipc hub open colors/history` opens the page.

## In the launcher

Type `#` in the [launcher](launcher.md). Alone, it offers "Pick a color from the screen" and lists the history. Followed by a color, it lists that color in every format; Enter copies the one you choose, Shift+Enter types it into the window you were in, and the color goes into the history. It reads:

- hex: `1e1e2e`, `#1e1e2e`, `#abc`, or with transparency, `#1e1e2e80`
- `rgb(30 30 46)`, `rgb(30, 30, 46)` and `rgba()`
- `hsl(240 21% 15%)`, `hsl(240, 21%, 15%)` and `hsla()`
- `oklch(24% 0.03 284)`
- CSS color names, like `rebeccapurple`

Anything else lists the colors in the history and the CSS names that start with it.

```toml
{{#include ../../../../modules/colors/settings.toml}}
```

## Actions

| Action | What it does |
|---|---|
| `pick` | Opens the picker |
| `cancel` | Closes the picker |
| `copy <color> [format]` | Copies a color, like `#1e1e2e`, in the default format or in `hex`, `rgb`, `hsl` or `oklch` |
| `remove <color>` | Removes a color from the history |
| `clear` | Removes every color from the history |

The hub sends `start`, the launcher `search` and `pick-result`, and the picker `hover` and `select` themselves.

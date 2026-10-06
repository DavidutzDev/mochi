# Widgets

Widgets are small views on the desktop, under your windows: a clock, and whatever other modules and plugins offer. You can place as many as you like, the same one several times, each with its own settings, on any monitor.

## Arranging them

`mochi ipc widgets edit` brings the widgets over the windows on the focused monitor, dims the screen and shows the grid and a drawer:

- Drag a widget to move it. It snaps to the grid.
- Drag its round corner handle to resize it, within the sizes it allows.
- Click it to open its settings, a form made from the settings it declares. Changes apply at once.
- Drag a widget from the drawer onto the screen to add it, or click its trash button to remove it.
- "Copy as Nix" copies the layout for home-manager.
- Done or Escape stops arranging.

Bind it to a key, like the other panels:

```lua
hl.bind("SUPER + W", hl.dsp.exec_cmd("mochi ipc widgets edit"))
```

## widgets.toml

The layout is in `widgets.toml`, next to `config.toml`. Arranging rewrites it after every change, and changes you make to it by hand apply as soon as you save it; a file that doesn't read is logged, and the widgets stay as they were. `mochi config check` checks it.

```toml
[[widget]]
id = "w1"              # its own, made when it's added
module = "widgets"     # the module offering it
widget = "clock"       # which of its widgets
output = "DP-3"        # the monitor
anchor = "top-right"   # the point it's placed from
x = -2                 # grid cells from that point
y = 3
width = 14             # in grid cells
height = 7

[widget.settings]
timezone = "Asia/Tokyo"
seconds = true
```

A widget's `anchor` point sits at the same point of the screen, moved by `x` and `y` cells: a widget anchored `bottom-right` with `x = -2` stays two cells from the right edge on any screen size. The anchors are `top-left`, `top`, `top-right`, `left`, `center`, `right`, `bottom-left`, `bottom` and `bottom-right`. Arranging picks the anchor of the third of the screen a widget's middle is in.

A widget whose module isn't running, or whose monitor isn't connected, waits in the file until it is.

## With home-manager

`programs.mochi.widgets` declares a layout, as a list like `mochi ipc widgets export` prints, or as the text of a `widgets.toml`:

```nix
programs.mochi.widgets = [
  {
    id = "w1";
    module = "widgets";
    widget = "clock";
    output = "DP-3";
    anchor = "top-left";
    x = 2;
    y = 4;
    width = 16;
    height = 8;
    settings.seconds = true;
  }
];
```

It's written as a file Mochi can change, not a link into the store, so arranging keeps working. A rebuild writes it again only when the declared layout changed, so what you arranged survives other rebuilds. To keep an arrangement for good, use "Copy as Nix" while arranging, or `mochi ipc widgets export`, and paste it in place of the old layout.

```toml
{{#include ../../../../modules/widgets/settings.toml}}
```

## The clock

| Setting | Default | |
|---|---|---|
| `timezone` | `""` | A zone like `Europe/Paris`; empty for this computer's |
| `hours` | `"24"` | `"24"` or `"12"` |
| `seconds` | `false` | Show the seconds |
| `date` | `true` | Show the date under the time |

## Actions

| Action | What it does |
|---|---|
| `edit [on\|off\|toggle]` | Starts or stops arranging, on the focused monitor |
| `add <module> <widget> [output] [anchor] [x] [y]` | Places a widget; prints its id |
| `move <id> <output> <anchor> <x> <y>` | Moves one |
| `resize <id> <width> <height>` | Resizes one, in cells, within its limits |
| `set <id> <setting> <value>`, `reset <id> <setting>` | Changes one of its settings, or puts it back to the default |
| `remove <id>` | Removes one |
| `export [nix\|toml]`, `copy [nix\|toml]` | Prints the layout, or copies it |

Modules and plugins offer widgets of their own: see [Writing widgets](../writing-widgets.md).

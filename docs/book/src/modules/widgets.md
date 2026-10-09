# Widgets

Widgets are small views on the desktop, under your windows, from any module or plugin. You can place as many as you like, the same one several times, each with its own settings, on any monitor.

## Arranging them

`mochi ipc widgets edit` brings the widgets over the windows on the focused monitor, dims the screen and shows the grid, and the island says "Arranging widgets":

- Drag a widget to move it. It snaps to the grid.
- Drag its round corner handle to resize it, within the sizes it allows.
- While you drag or resize, thin accent lines show where the widget's edges or middle line up with another widget's edges or middle, or with the middle of the screen. Within 6 pixels of such a line, the widget snaps to it instead of the nearest grid cell. Positions are saved as whole cells from the widget's anchor, so it only snaps to lines it can be saved on: a widget whose middle is in the middle third of the screen counts its cells from the screen's middle, so it can center on the screen but not always line up its edge with a widget on the left.
- Click it, or its pencil, to open its settings, a form made from the settings it declares. Changes apply at once. Its trash button removes it.
- With more than one monitor, its settings end with a choice of monitor, left to right. Pick another to send the widget there: it keeps its anchor and its offsets, moved in only as far as it takes to stay on that screen, and arranging moves to that monitor with the widget's settings open.
- Where widgets overlap, the one on the higher layer is on top. The arrows over a widget move it a layer up or down.
- Click the island to open the drawer under it: every widget the running modules and plugins offer, a search box, and a filter per module. Drag one onto the screen to add it; the drawer folds out of the way as you drag, and when the pointer leaves it.
- Copy, at the bottom of the drawer, copies the layout as `widgets.toml`; its arrow has "Copy as Nix", for home-manager.
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
z = 1                  # its layer, 0 when left out: higher is on top

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

## The widgets

| Widget | `module`, `widget` | Settings |
|---|---|---|
| Clock | `widgets`, `clock` | `timezone` (like `Europe/Paris`; empty for this computer's), `hours` (`"24"` or `"12"`), `seconds`, `date` |
| Calendar | `widgets`, `calendar` | `first_day` (`monday` or `sunday`). Arrows go to the months around this one |
| To-do | `notes`, `todo` | `title`, `done` (`show` or `hide` the done items). Click an item to tick it; type into the bottom field and press Enter to add one |
| Note | `notes`, `note` | `title`. Click it and type; it saves a moment after you stop |
| Now playing | `media`, `now-playing` | The control center's card: the cover, the track and the controls. It steps aside while no player has a track |
| Battery | `battery`, `level` | The control center's card, with the power profiles. It steps aside without a battery |
| Performance | `performance`, `graphs` | `cpu`, `memory`, `gpu` (on unless set off), `disk`, `network` (off unless set on): which readings show, each with its last two minutes as a graph. An older widget with `reading` set to one reading in `widgets.toml` shows only that one until you remove the line |
| Palette | `colors`, `palette` | The latest [colors](colors.md) you picked, as many as fit. Click one to copy it, hover it to see it |
| Weather | `weather`, `now` | From the example weather plugin |

Each to-do list and note keeps its own content, in the [notes](notes.md) module. Typing into one gives the desktop the keyboard until you click a window; Escape lets go of the field.

## Actions

| Action | What it does |
|---|---|
| `edit [on\|off\|toggle] [output] [id]` | Starts or stops arranging, on the focused monitor or the one named, with a widget's settings open |
| `add <module> <widget> [output] [anchor] [x] [y]` | Places a widget; prints its id |
| `move <id> <output> <anchor> <x> <y>` | Moves one |
| `resize <id> <width> <height>` | Resizes one, in cells, within its limits |
| `set <id> <setting> <value>`, `reset <id> <setting>` | Changes one of its settings, or puts it back to the default |
| `remove <id>` | Removes one |
| `layer <id> up\|down\|front\|back` | Moves one a layer up or down, or over or under all the others |
| `drawer [on\|off\|toggle]` | Opens or closes the drawer while arranging |
| `export [nix\|toml]`, `copy [nix\|toml]` | Prints the layout, or copies it |

Modules and plugins offer widgets of their own: see [Writing widgets](../writing-widgets.md).

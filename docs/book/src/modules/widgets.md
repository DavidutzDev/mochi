# Widgets

Widgets are small views on the desktop, under your windows, from any module or plugin. You can place as many as you like, the same one several times, each with its own settings, on any monitor.

## Arranging them

`mochi ipc widgets edit` brings the widgets over the windows on the focused monitor, dims the screen and shows the grid, and the island says "Arranging widgets":

- Drag a widget to move it. It snaps to the grid.
- Drag its round corner handle to resize it, within the sizes it allows.
- While you drag or resize, thin accent lines show where the widget's edges or middle line up with another widget's edges or middle, or with the middle of the screen. Within 6 pixels of such a line, the widget snaps to it instead of the nearest grid cell. Positions are saved as whole cells from the widget's anchor, so it only snaps to lines it can be saved on: a widget whose middle is in the middle third of the screen counts its cells from the screen's middle, so it can center on the screen but not always line up its edge with a widget on the left.
- Click it, or its pencil, to open its settings, a form made from the settings it declares. Changes apply at once. Its trash button removes it.
- With more than one monitor, its settings end with a choice of monitor, left to right. Pick another to send the widget there: it keeps its anchor and its offsets, moved in only as far as it takes to stay on that screen, and arranging moves to that monitor with the widget's settings open.
- A widget with several looks, like a clock that can be digital or stacked, has them at the top of its settings. A new look comes at its own size, from the same anchor.
- Where widgets overlap, the one on the higher layer is on top. The arrows over a widget move it a layer up or down.
- Click the island to open the drawer, a panel down the left side of the screen. It goes on the right when the island sits on the left, so the island's notice stays clear. Its close button or Escape closes it. It has three tabs:
  - Add: every look of every widget the running modules and plugins offer, as cards with a live preview, the widget's own view with its real data, a line on what it shows, and how many are on the desktop already. A search box and a chip per category narrow the list. Click a card to put it in the first free spot on this screen, clear of the panel and the island, or drag it onto the screen to put it there; the panel slides away while you drag.
  - On desktop: every widget placed, on every monitor, with where it is. Its pencil opens its settings, on the monitor it's on, and its trash button removes it.
  - Layouts: arrangements saved under a name. See [Saved layouts](#saved-layouts).
- Copy, at the bottom of the drawer, copies the layout as `widgets.toml`; its arrow has "Copy as Nix", for home-manager.
- Done stops arranging. So does Escape, once the drawer is closed.

Tab moves through the drawer's search, chips and cards, and Enter or Space adds the card that has the focus; the arrow keys switch the tabs while they have it.

Bind it to a key, like the other panels:

```lua
hl.bind("SUPER + W", hl.dsp.exec_cmd("mochi ipc widgets edit"))
```

## widgets.toml

The layout is in `widgets.toml`, next to `config.toml`. Arranging rewrites it after every change, and changes you make to it by hand apply as soon as you save it; a file that doesn't read is logged, and the widgets stay as they were. `mochi config check` checks it.

```toml
layout = "Work"        # the saved layout this is, if any

[[widget]]
id = "w1"              # its own, made when it's added
module = "widgets"     # the module offering it
widget = "clock"       # which of its widgets
variant = "digital"    # which of its looks; the first when left out
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

A widget whose module isn't running, or whose monitor isn't connected, waits in the file until it is. One whose `variant` its widget no longer offers gets the first look.

## Saved layouts

The drawer's Layouts tab keeps arrangements under a name, to switch between, like one for work and one for the evening. Type a name and press Save, or Enter: the arrangement on the desktop is kept under that name, replacing one saved under it, and from then on it is that layout, so the changes you make go to it too. Use puts another in its place. An arrangement without a name that isn't saved already is kept as "Unsaved" when you switch away from it, so switching never loses widgets. The trash button deletes a saved layout, after a second click on Delete; deleting the one on the desktop leaves its widgets there, without a name.

Each is a file in `widget-layouts/`, next to `widgets.toml`, like `widget-layouts/Work.toml`, in the same form as `widgets.toml`. Copying one into another machine's `widget-layouts/` brings it there. A widget keeps its id in every layout it's saved in, so a note saved in two layouts is the same note, and removing a widget keeps its content while a saved layout still has it.

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
| Clock | `widgets`, `clock` | Looks: `digital`, the time over the date; `stacked`, the hour over the minutes; `analog`, hands and ticks; `shape`, the time inside a cookie in the accent color; `minimal`, the time and the date on one line, without a card; `world`, the time in a few cities. `timezone` (like `Europe/Paris`; empty for this computer's), `hours` (`"24"` or `"12"`), `seconds` (digital and analog), `date` (digital, stacked and minimal), `zones` (world) |
| Calendar | `widgets`, `calendar` | Looks: `month`, this month as a grid, with arrows to the months around it; `week`, this week on a strip. Today is a pentagon in the accent color. `first_day` (`monday` or `sunday`) |
| Focus timer | `timer`, `focus` | The [timer](timer.md)'s time left in a ring, with pause and stop, or Start focus when nothing runs |
| To-do | `notes`, `todo` | `title`, `done` (`show` or `hide` the done items). Click an item to tick it; type into the bottom field and press Enter to add one |
| Note | `notes`, `note` | `title`. Click it and type; it saves a moment after you stop |
| Now playing | `media`, `now-playing` | Looks: `card`, the control center's card, the cover beside the track and the controls; `artwork`, a tall card with the cover big, the track, the progress and the controls under it; `cover`, only the cover, without a card, with a button to play or pause. It steps aside while no player has a track |
| Battery | `battery`, `level` | Looks: `card`, the control center's card, with the power profiles and the peripherals' batteries; `ring`, the level as a ring, and whether it's charging and how long until full or empty. The card steps aside without a battery or a peripheral with one, the ring without a battery |
| Performance | `performance`, `graphs` | Looks: `graphs`, each reading with its last two minutes; `rings`, CPU, memory and disk as rings; `meters`, a bar for CPU, memory, GPU and disk, with what each uses. The disk is the one your home directory is on. For the graphs, `cpu`, `memory`, `gpu` (on unless set off), `disk`, `network` (off unless set on): which readings show. An older widget with `reading` set to one reading in `widgets.toml` shows only that one until you remove the line |
| System info | `performance`, `system` | The distribution, the kernel, the uptime, Mochi's version, the compositor, the CPU and the memory in use, as many lines as fit |
| Palette | `colors`, `palette` | The latest [colors](colors.md) you picked, as many as fit. Click one to copy it, hover it to see it |
| Weather | `weather`, `current` | Looks: `current`, the [weather](weather.md) now with the place; `icon`, the sky's icon in a cookie with the temperature beside it; `forecast`, the next days with their low and high; `hours`, the next 12 hours as a curve. It says how to set a place until one is set |

The world clock's `zones` are names like `Europe/London`, with commas between them, up to four; in `widgets.toml` they can be a list too. Quickshell has no time zone database, so the module asks the system for each zone's offset from UTC, with `date`, and again every ten minutes, so a change to daylight saving shows within ten minutes. A zone the system doesn't have says so on its line. Each line says how far ahead of this computer the city is, and "Tomorrow" or "Yesterday" when its date isn't today's here.

The minimal clock has no card: a soft halo in the island's background color sets it apart from the wallpaper. With software rendering, which has no effects, it shows without the halo.

Each to-do list and note keeps its own content, in the [notes](notes.md) module. Typing into one gives the desktop the keyboard until you click a window; Escape lets go of the field.

## Actions

| Action | What it does |
|---|---|
| `edit [on\|off\|toggle] [output] [id]` | Starts or stops arranging, on the focused monitor or the one named, with a widget's settings open |
| `add <module> <widget>[:<variant>] [output] [anchor] [x] [y]` | Places a widget, in a look like `clock:digital` or its first; prints its id. Without an anchor, in the first free spot, down the left edge first |
| `variant <id> <variant>` | Gives one another look, at that look's size |
| `move <id> <output> <anchor> <x> <y>` | Moves one |
| `resize <id> <width> <height>` | Resizes one, in cells, within its limits |
| `set <id> <setting> <value>`, `reset <id> <setting>` | Changes one of its settings, or puts it back to the default |
| `remove <id>` | Removes one |
| `layer <id> up\|down\|front\|back` | Moves one a layer up or down, or over or under all the others |
| `drawer [on\|off\|toggle]` | Opens or closes the drawer while arranging |
| `save-layout <name>`, `use-layout <name>`, `delete-layout <name>` | Saves the arrangement under a name, puts a saved one in its place, or deletes one |
| `layouts` | Prints the saved layouts |
| `export [nix\|toml]`, `copy [nix\|toml]` | Prints the layout, or copies it |

The desktop layer sends `screen <output> <width> <height> [left] [right] [top] [bottom]` itself: each monitor's size, and what the drawer and the island cover, for finding free spots.

Modules and plugins offer widgets of their own: see [Writing widgets](../writing-widgets.md).

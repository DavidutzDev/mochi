# Writing widgets

Any module or plugin can offer widgets for the [desktop](modules/widgets.md). A widget is a view, offered with a contribution to the `widgets` module; users place it as many times as they like, and each placed one, an instance, has its own id and settings.

## The contribution

In a plugin's manifest:

```toml
[[contributions]]
target = "widgets"
kind = "widget"
id = "countdown"
view = "Countdown"
title = "Countdown"
icon = "clock"
options = { size = [12, 6], min = [8, 4], max = [24, 12], category = "Calendar",
    description = "The days left until a date", settings = [
    { name = "label", default = "Holidays", description = "What it counts down to" },
    { name = "date", default = "2026-12-20", description = "The day, as YYYY-MM-DD" },
    { name = "big", kind = "bool", default = false, description = "Only the number" },
] }
```

A builtin module returns the same as a `ContributionSpec` from `Module::contributions`, with the options as JSON.

| Option | Default | |
|---|---|---|
| `size` | `[12, 8]` | The size it's added at, in grid cells |
| `min`, `max` | `[2, 2]`, `[200, 120]` | How small and large users can make it |
| `frame` | `true` | Whether Mochi draws the card behind it. `false` for a widget that floats on the wallpaper |
| `settings` | `[]` | What users can set for each instance: `name`, `kind` (`string`, `int`, `float`, `bool` or `choice` with `choices`), `default` and `description`. The editor makes a form from them |
| `forget` | | An action the module runs with an instance's id when the user removes it, to drop what it kept for it |
| `category` | the module's name | What the drawer groups it under, like `Clock` or `System`. Widgets of several modules can share one |
| `description` | | One line on what users see, for the drawer's card, like "The cover beside the track, with its controls". Plain words, under about 50 characters, so it fits two lines |
| `variants` | `[]` | Its looks; see below |

## Variants

A widget can have several looks, like a clock that's digital, stacked or analog. Each is a variant: the drawer shows a card for each, users pick one there and can change it in the widget's settings, and the placed widget records it in `widgets.toml`.

```toml
options = { size = [14, 7], min = [8, 4], category = "Clock", settings = [
    { name = "seconds", kind = "bool", default = false, description = "Show the seconds" },
    { name = "hand", default = "", description = "A color for the second hand" },
], variants = [
    { id = "digital", title = "Digital", description = "The time, big, over the date" },
    { id = "stacked", title = "Stacked", description = "Hour above minute, like a headline",
      size = [9, 12], min = [6, 8], settings = ["seconds"] },
    { id = "analog", title = "Analog", description = "Hands and ticks on a round face",
      view = "Analog", size = [12, 12], min = [8, 8], max = [30, 30] },
] }
```

| Key | | |
|---|---|---|
| `id` | required | One word, without a colon: `widgets.toml` and `add <module> <widget>:<id>` name it |
| `title` | required | Its name on the card and in the settings, like "Stacked" |
| `description` | `""` | One line on what users see; the card shows it instead of the widget's |
| `view` | the widget's | A view of its own, for a look that shares little with the others |
| `size`, `min`, `max` | the widget's | Its sizes; a widget switched to it takes its `size` |
| `frame` | the widget's | Whether Mochi draws the card behind it, so one look can float on the wallpaper, like the minimal clock |
| `settings` | all of them | The names of the widget's settings that apply to it; the form leaves out the others |

The first variant is the default: a widget placed before its widget had variants, or placed with one its widget no longer offers, gets it. So when you add looks to a widget that had one, describe the look it has now first, and add the others after it. A widget with variants keeps one description per variant, and doesn't need its own.

Adding a look is one more entry in `variants` and, when it doesn't name a view of its own, one more branch in the view that reads `variant`. A look's settings keep their values when the user switches to another look, so a view reads only the ones its look lists: the stacked clock leaves `seconds` alone even when the digital one had it on.

The builtin looks use pieces from the core, in `qs.island`: `ExpressiveShape`, a circle, pentagon, cookie, clover or burst to set one thing apart; `WavyRing`, how full something is as a ring; `WavyProgress`, a progress line that waves while something plays; and `StackedTime`, a big clock's hour over its minutes. A shape is an accent: one per widget, behind the thing that matters, like today on the calendar.

## The view

The view fills the widget, inside the card's padding when it has a card. It gets:

| Property | |
|---|---|
| `payload` | The offering module's published state, as control center cards get it |
| `settings` | This instance's settings, with the defaults filled in |
| `instance` | This instance's id, like `w3`, or `preview` in the drawer's cards |
| `variant` | The id of its look, for a view that declares `property string variant`. It comes right after the view is made, so bind to it rather than reading it once in `Component.onCompleted` |

```qml
import QtQuick
import qs.island

Item {
    property var payload: null
    property var settings: ({})
    property string instance: ""

    Text {
        anchors.centerIn: parent
        text: settings.label
        color: Theme.foreground
        font.pixelSize: Math.min(parent.height * 0.4, Theme.textDisplay)
    }
}
```

A view with looks declares `property string variant: ""` and picks what to draw from it, with the first look for `""`:

```qml
Item {
    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool stacked: variant === "stacked"
}
```

Size text from the widget's size, since users resize it. The drawer shows the real view, with the module's real state and the default settings, scaled down and taking no clicks, as the preview on each card, with `instance` set to `preview`: a view keeping content per instance shows none there. Set `property bool hidden: true` to step aside while there's nothing to show; the widget comes back in edit mode, and the drawer's card says there's nothing to show right now. A view with a text field sets `property bool typing: true`: the desktop then takes the keyboard when the user clicks into it, and gives it back on a click elsewhere. Let Escape take the focus off the field.

A control center card's view often works as a widget as is: media and battery offer their cards both ways.

## Instance content

A widget that keeps content per instance, like a note's text, keeps it in its module, keyed by `instance`: it publishes it in its state, by instance, and its view changes it with actions that take the instance's id, like `Daemon.command("notes", "write", [instance, text])`. Name an action in `forget` to drop an instance's content when the user removes it.

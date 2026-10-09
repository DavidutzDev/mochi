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
options = { size = [12, 6], min = [8, 4], max = [24, 12], settings = [
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

## The view

The view fills the widget, inside the card's padding when it has a card. It gets:

| Property | |
|---|---|
| `payload` | The offering module's published state, as control center cards get it |
| `settings` | This instance's settings, with the defaults filled in |
| `instance` | This instance's id, like `w3` |

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

Size text from the widget's size, since users resize it. Set `property bool hidden: true` to step aside while there's nothing to show; the widget comes back in edit mode. A view with a text field sets `property bool typing: true`: the desktop then takes the keyboard when the user clicks into it, and gives it back on a click elsewhere. Let Escape take the focus off the field.

A control center card's view often works as a widget as is: media and battery offer their cards both ways.

## Instance content

A widget that keeps content per instance, like a note's text, keeps it in its module, keyed by `instance`: it publishes it in its state, by instance, and its view changes it with actions that take the instance's id, like `Daemon.command("notes", "write", [instance, text])`. Name an action in `forget` to drop an instance's content when the user removes it.

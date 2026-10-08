# Tour

A guided tour of Mochi. Each module shows its real views with made-up data, so nothing of yours appears on screen, and a caption says what it does and where to find it.

The first time Mochi starts, a notice on the island offers the tour: **Take the tour**, **Later**, which asks again next time, or **Never**. After an update, it offers a shorter tour of what's new since your last one, and nothing at all when nothing you'd see changed. `mochi ipc tour start` runs it any time, as do "Tour" and "What's new in Mochi" in the launcher and **Take the tour** on the tour's page in Settings.

## During the tour

- The island is paused: notifications, the volume and every other module's notices wait, and their bubbles hide. They all come back when the tour ends.
- Every monitor dims, and the dim layer takes every click and key, so nothing behind it reacts. Your compositor's own keybinds still work.
- The tour moves on by itself, a few seconds a step. Space or → goes on sooner, ← goes back.
- Escape asks whether to stop: Enter stops, Escape carries on. A stopped tour starts again from the same step next time.
- Steps that show a look of the island, like notch mode or the bottom edge, try it without saving it, and put yours back after.

A module that's off gets a card saying so, and where to turn it on.

```toml
{{#include ../../../../modules/tour/settings.toml}}
```

`$XDG_STATE_HOME/mochi/tour.json` keeps the last release whose tour you saw, and where a stopped tour was.

## Actions

| Action | |
|---|---|
| `start [all\|new]` | The whole tour, or what's new since the last one; by default the one offered, or where it stopped. |
| `next`, `back` | Moves through the steps. |
| `stop`, `continue`, `end` | Asks whether to stop, carries on, or stops at once. |
| `later`, `never` | Answers the offer. |

## Steps from modules and plugins

Each module offers its steps as contributions to the tour, so the tour of a module lives with its code. A plugin can do the same in its manifest:

```toml
[[contributions]]
target = "tour"
kind = "step"
id = "now"
view = "Card"
title = "Weather"
icon = "wb_sunny"
order = 10
options = { chapter = "desktop", since = "0.1.0", place = "card", size = [280, 96], caption = "The weather where you are.", payload = { temperature = 18, sky = "clear" } }
```

| Option | |
|---|---|
| `chapter` | `island`, `panels`, `notices`, `desktop`, `capture` or `settings`. |
| `since` | The release that brought it: the tour of what's new shows it to anyone whose last tour was before. |
| `caption` | One or two plain sentences. |
| `payload` | The made-up data the view gets. |
| `place` | `island` (the default), `bubble`, or a hub `card` or desktop `widget` framed at `size`. A bubble's payload can name its `area`. |
| `properties` | More properties the view takes, like a widget's `settings` and `instance`. |

The view is loaded with interaction turned off, and only when the module runs.

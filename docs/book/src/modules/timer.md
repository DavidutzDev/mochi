# Focus timer

A focus timer, after the pomodoro technique: 25 minutes of focus, then a 5-minute break, and a 15-minute break after every fourth session. Start it from its card in the [control center](control-center.md), or with `mochi ipc timer start`, which you can bind to a key. Custom timers run beside it, like a kitchen timer's: several at once, each with a label.

Inspired by [mochi-clock](https://github.com/Xonex5/mochi-clock) by Xonex5: custom timers, their natural lengths, the alarm and the launcher's `:t` come from it. The bottom of the timer's page in the settings says so, and **See repo** there opens it.

While it runs, a bubble by the island counts down: a ring that empties as the time goes, with the minutes left inside it, in the accent color for focus and in green for a break. Rest the pointer on it for the time to the second. A click pauses it or resumes it. With `wide = true` in `[bubbles.timer]`, the bubble shows the time to the second, like 18:42.

When focus ends, the island says so and offers the break, with Start the break and Done. With `auto_break = true` the break starts by itself and the notice says so, with Skip the break to go straight back to focus. When a break ends, the island offers the next session. The card in the control center has the time left with pause and stop, or Start when nothing runs. The [desktop](widgets.md) has a focus timer widget: the time left big inside a ring that waves while it counts down, with pause and stop, or Start focus when nothing runs.

The timers carry on when mochid restarts, like after an update, and changing the settings doesn't stop them: the new lengths apply from the next phase. A phase or a timer that ran out while the computer slept ends as soon as it wakes. Logging out forgets them.

## Custom timers

`mochi ipc timer add 15m Tea` starts a 15-minute timer labelled Tea. A length is written the way people type it: `5m`, `90s`, `1h 30m`, `1h30`, `1.5h`, `2 min 30 sec`, `10:00` or `1:30:00`, or a bare number of minutes like `25`. Whatever follows the length is the label. A number without a unit after one with a unit takes the next unit down, so `1h 30` is an hour and a half. Up to 20 run at once, each up to a day.

The clock panel's [Timer tab](clock.md#timer) lists them with their time left and when each ends, with "+1 min", pause and stop, and has a field to start one and a button for each of `presets`. The launcher starts them too: see below.

Each running timer has its own bubble, in the text color so it stands apart from the focus session's accent, side by side in one pill. Its last minute counts in seconds. A click pauses or resumes that timer; with `wide = true` the bubble shows the label and the time to the second. `timer_bubbles = "soonest"` shows one bubble instead, for the timer that ends first, with how many more run; `"off"` shows none.

When a timer runs out, the island says so with its label, and offers Again, "+1 min" and Done. The alarm plays, `alarm-clock-elapsed` from the sound theme or your `sound_file`, at `volume`, through `pw-play` or `paplay`, or `canberra-gtk-play` for the theme's sound; `sound_command` plays it with a program of your own. Music or a video playing pauses, through the [media](media.md) module, unless `pause_media = false`. The timer's page in the settings has a button that plays the alarm, to try the volume and the file, and **Choose…** beside `sound_file` picks the file with the desktop's file chooser, showing sound files. The clock panel's [Timer tab](clock.md#timer) has the same along its bottom: the volume, the sound with **Choose…** and **Default**, and **Play it**. `focus_sound = true` plays it when a focus session or a break ends too.

## In the launcher

After `:t ` (with the space, so emoji like `:tea` still work), the [launcher](launcher.md) offers to start the timer you type: `:t 10m Pizza` starts a 10-minute timer labelled Pizza. With nothing typed it lists the running timers, to pause or resume, then `presets`. `:t sw` starts or pauses the [clock](clock.md)'s stopwatch, and offers a lap while it runs or a reset when it's paused. `[module.launcher.providers.timer]` changes the prefix, like `prefix = "t "`.

| Action | What it does |
|---|---|
| `start [minutes]` | Starts a focus session, of `focus_minutes` or the minutes given |
| `break [minutes]` | Starts a break, long or short by the sessions done, or of the minutes given |
| `pause [id]`, `resume [id]` | Pause the focus session, or carry on from where it paused. With a custom timer's number from `list`, or `all`, that timer or all of them |
| `toggle [id]` | Pauses or resumes, or starts a focus session when nothing runs. With a number, pauses or resumes that custom timer |
| `stop [id]` | Stops the focus session, or a custom timer by its number, or `all` of them |
| `status` | Prints the phase and the time left, or the sessions done |
| `add <length> [label]` | Starts a custom timer, like `add 15m Tea` or `add 1h 30m Bread` |
| `extend <id> [length]` | Adds `extend_seconds`, or the length given, to a custom timer or `all` |
| `list` | Prints the custom timers, a line each: the number, the label and the time left |
| `test-sound` | Plays the alarm |

```toml
{{#include ../../../../modules/timer/settings.toml}}
```

For 50 minutes of focus and 10 of rest, breaks starting by themselves:

```toml
[module.timer]
focus_minutes = 50
break_minutes = 10
auto_break = true
```

A quieter alarm of your own, one bubble for the custom timers, and the music left playing:

```toml
[module.timer]
sound_file = "~/sounds/bell.oga"
volume = 40
timer_bubbles = "soonest"
pause_media = false
```

# Clock

A panel on the island with five tabs: Today, Calendar, Timer, Stopwatch and World. `mochi ipc clock toggle`, bound to a key, opens and closes it; `mochi ipc clock open calendar` opens it on a tab, or switches to that tab while it's open. A click on the Today card in the [control center](control-center.md) opens it too. Opening it closes the other panels, like the launcher or the control center, and Escape closes it.

The keyboard reaches everything: Tab goes through the controls and the navbar, 1 to 5 pick a tab, Left and Right move along the navbar, and Enter or Space press what has the focus. In the calendar's month, the arrows move the picked day, Page Up and Page Down turn the month, Home goes back to today, and Enter goes to the field for a new reminder.

To open it with a click on the idle clock instead of the control center:

```toml
[module.idle]
click = ["clock", "toggle"]
```

## Today

The time big inside a shape in the accent color, with the date above it and a greeting under it. `shape` picks the shape: a cookie by default, or a circle, pentagon, clover, burst, hexagon, octagon, squircle or pill, or none for the time alone in the text's color. With `seconds` on, the seconds sit under the minutes. `day_progress` shows how far through the day it is: a thin line under the date with the share beside it, a ring around the shape, or neither. Beside it, the weather from the [weather module](weather.md): the sky and the temperature now with the place and today's low and high, the temperature it feels like, the humidity, the wind and the UV index, then the next 8 hours and the next 5 days. Until a place is set, it says so, and **Set a place** opens the setting; with the weather module off, a button opens its settings.

Along the bottom, the next reminder and when it's due, or the oldest one still due, with **Done**. **Add a reminder** opens the calendar on today with the field ready.

## Calendar and reminders

A month, today in an accent pentagon and a dot under each day with reminders, a full one while one of them isn't done. Picking a day lists its reminders on the right, by time: **Done** for one that's due, and the bin to delete it. Under them, a time and a text add one to the picked day. The time takes `18:30`, or `6:30 PM` and `6 pm` on a 12-hour clock.

When a reminder comes due, the island says so, with **Snooze 10 min** and **Done**. Left alone, the notice goes after a minute, and the reminder stays due on the Today tab and in the calendar until you mark it done. Reminders go by the wall clock, so one that came due while the computer slept comes up within seconds of waking, and one that came due while mochid wasn't running comes up when it starts, with how long ago it was due.

`$XDG_STATE_HOME/mochi/reminders.json` keeps them, done ones too, so the calendar still shows them. Each has a number, which the actions take:

```sh
mochi ipc clock remind tomorrow 19:30 "Ana's birthday dinner"
mochi ipc clock remind 2026-10-21 09:00 Dentist
mochi ipc clock reminders            # every one, with its number
mochi ipc clock reminders tomorrow   # one day's
mochi ipc clock done 3
mochi ipc clock delete 3
```

## Timer

The [focus timer](timer.md)'s countdown in a ring that waves while it runs, with lengths to start focus or a break, and pause, resume and stop while one runs. The timer module does the counting, so its bubble and its notices work as always. With the timer module off, the tab says so and opens its settings.

## Stopwatch

The time to the tenth of a second, or the hundredth with `precision = "hundredths"`, with **Start**, **Lap**, **Pause** and **Reset**. On the right, the laps, newest first: how long each took next to its number, the time at the end of it, and the fastest and the slowest marked. The clock module keeps the stopwatch, so it runs on with the panel closed and through a restart of mochid, like after an update; logging out forgets it. It keeps up to 99 laps.

**Reset** keeps the run: **Runs** beside **Laps** lists the past ones, newest first, with when each ended, its time and how many laps it had. `history` sets how many it keeps, 10 by default and up to 50, or 0 for none; `$XDG_STATE_HOME/mochi/stopwatch-runs.json` keeps them, so they outlast a logout. **Copy** hands the run going to the clipboard, with its laps, and the copy button on a past run hands over that one; **Clear** forgets them all. Copying goes through the [clipboard module](clipboard.md), or `wl-copy` when it's off:

```text
Stopwatch, 2026-10-09 23:47: 0:02.6
Lap 1  0:01.3  0:01.3
Lap 2  0:00.8  0:02.1
```

## World

The time here, then in each of the `zones`, two to a row: the city, whether it's yesterday, today or tomorrow there, how many hours ahead or behind it is, and the time. The zones' offsets come from the system's time zone database, read again every 10 minutes for daylight saving, as for the world clock widget. A zone the system doesn't have says so.

**Add a city** lists common cities from Honolulu to Auckland, west to east: a click adds one, or takes off one that's there, until the tab has its 8. Escape or **Done** goes back to the cards. The list writes the `zones` setting, as the settings panel does, and **Change the zones** opens it there, for any zone the system knows.

## In the control center

With the [control center](control-center.md) on, the clock offers a card for its home. It waits under More cards until you put it there, since the control center's own Today card already shows the time. The card has the time, then the stopwatch while it has time on it, with a button to pause or resume it, or the next reminder and when it's due. Its heading opens the Clock page, which has the panel's Today, Calendar, Stopwatch and World tabs, the same views, on a row at the top. The focus timer has a card of its own there.

## Credit

The seconds and the day's progress on Today, the stopwatch's tenths and past runs, copying a run, and the list of cities come from [mochi-clock](https://github.com/Xonex5/mochi-clock), a plugin by [Xonex5](https://github.com/Xonex5). The clock's page in the settings says so, and **Open on GitHub** there opens it.

| Action | What it does |
|---|---|
| `toggle` | Opens the panel on the `tab` setting's tab, or closes it |
| `open [tab]` | Opens the panel, on `today`, `calendar`, `timer`, `stopwatch` or `world`; switches tabs while it's open |
| `close` | Closes the panel |
| `remind <date> <time> <text>` | Adds a reminder; the date is like `2026-10-21`, `today` or `tomorrow`, the time like `19:30` |
| `reminders [date]` | Prints every reminder, or a day's, with its number |
| `done <id>` | Marks a reminder done, so it doesn't come up again |
| `snooze <id> [minutes]` | Brings a reminder up again in 10 minutes, or the minutes given |
| `delete <id>` | Deletes a reminder |
| `stopwatch <what>` | `start`, `pause`, `toggle`, `lap` or `reset` the stopwatch, or `status` to print its time and laps; `reset` keeps the run |
| `runs` | Prints the stopwatch's past runs, with their numbers |
| `copy-run [run]` | Copies the run going with its laps, or a past one by its number |
| `forget-run [run]` | Forgets a past run, or all of them |
| `add-zone <zone>`, `remove-zone <zone>` | Adds a time zone like `Europe/Paris` to the World tab, or takes one off, in the `zones` setting |
| `credit` | Opens mochi-clock's page on GitHub, which inspired the clock |

```toml
{{#include ../../../../modules/clock/settings.toml}}
```

Every setting applies at once, without closing the panel.

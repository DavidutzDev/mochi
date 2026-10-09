# Focus timer

A focus timer, after the pomodoro technique: 25 minutes of focus, then a 5-minute break, and a 15-minute break after every fourth session. Start it from its card in the [control center](control-center.md), or with `mochi ipc timer start`, which you can bind to a key.

While it runs, a bubble by the island counts down: a ring that empties as the time goes, with the minutes left inside it, in the accent color for focus and in green for a break. Rest the pointer on it for the time to the second. A click pauses it or resumes it. With `wide = true` in `[bubbles.timer]`, the bubble shows the time to the second, like 18:42.

When focus ends, the island says so and offers the break, with Start the break and Done. With `auto_break = true` the break starts by itself and the notice says so, with Skip the break to go straight back to focus. When a break ends, the island offers the next session. The card in the control center has the time left with pause and stop, or Start when nothing runs.

The timer carries on when mochid restarts, like after an update, and changing its settings doesn't stop it: the new lengths apply from the next phase. A phase that ran out while the computer slept ends as soon as it wakes. Logging out forgets it.

| Action | What it does |
|---|---|
| `start [minutes]` | Starts a focus session, of `focus_minutes` or the minutes given |
| `break [minutes]` | Starts a break, long or short by the sessions done, or of the minutes given |
| `pause`, `resume` | Pause the timer, or carry on from where it paused |
| `toggle` | Pauses or resumes, or starts a focus session when nothing runs |
| `stop` | Stops the timer |
| `status` | Prints the phase and the time left, or the sessions done |

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

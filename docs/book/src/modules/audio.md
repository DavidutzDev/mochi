# Audio

A volume mixer, like pavucontrol's: the output and the input with their volumes, and a slider and a mute for each app playing sound. It shows as the hub's Sound page, and `mochi ipc audio toggle`, bound to a key, opens the same mixer on the island; Escape or a click outside closes it. It needs PipeWire with its PulseAudio server, or PulseAudio.

Click the output's or the input's name to list the other devices, and click one to switch to it. Click an icon to mute or unmute. Apps playing come first; paused ones are dimmed. Each stream of an app has its own row, so two browser tabs playing get two sliders. Streams without a volume of their own, like some system sounds, aren't listed.

```toml
{{#include ../../../../modules/audio/settings.toml}}
```

| Action | What it does |
|---|---|
| `toggle`, `open`, `close` | Opens or closes the mixer on the island |
| `volume <target> <level>` | Sets a volume: `40` sets it, `+5` and `-5` move it, never past `max_volume` |
| `mute <target> [on\|off\|toggle]` | Mutes or unmutes; `toggle` when left out |
| `output <name>` | Plays sound through another output |
| `input <name>` | Records from another input |

A `target` is `output` or `input` for the devices in use, a device's name, or an app's stream id. `pactl list short sinks`, `sources` and `sink-inputs` list them. `mochi ipc audio volume output +5` makes a volume key.

`max_volume = 200` lets the sliders and `volume` go to 200%; the OSD's volume bar then shows 200% as full, with a mark at 100%. Past 100%, the sliders and the bar turn to the accent color. Volume keys bound to `mochi ipc audio volume output +5` stop at the same place; `wpctl set-volume -l` has its own limit.

Double-click a slider to put it back to 100%. A volume keeps each device's balance: the loudest channel goes to the level and the others follow. While the mixer or another panel is open, the OSD's volume notice doesn't show, and doesn't come back late once it closes. `mochi ipc hub open audio/mixer` opens the hub on the Sound page.

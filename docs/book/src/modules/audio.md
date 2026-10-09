# Audio

A volume mixer, like pavucontrol's: the output and the input with their volumes, and a slider and a mute for each app playing sound. It shows as the control center's Sound page, and `mochi ipc audio toggle`, bound to a key, opens the same mixer on the island; Escape or a click outside closes it. It needs PipeWire with its PulseAudio server, or PulseAudio.

Click the output's or the input's name to list the other devices, and click one to switch to it. Click an icon to mute or unmute. Apps playing come first; paused ones are dimmed. Streams without a volume of their own, like some system sounds, aren't listed.

The streams of one app share a row, so two browser tabs playing get one Firefox row. Its slider sets every stream to the same level, and its icon mutes them all; the row shows the loudest stream's volume, and shows muted only when all of them are. When an app has more than one stream, click its name to open the row and see each stream with its own slider, mute and output.

With more than one output, the button at the end of an app's row lists the outputs, with the one the app plays through marked, and a click moves every stream of the app there. In an opened row, each stream's button moves only that stream. PipeWire remembers where you moved an app: its next stream plays through the same output, without Mochi doing anything. `mochi ipc audio move firefox headphones` does the same from a keybind.

While the mixer shows, on the island or as the control center's Sound page, every slider has a meter: the fill dims, and the part of it the sound reaches stays lit. A meter is on the volume's scale, so it never goes past the slider's end: a full-scale sound fills the whole volume, a quiet one a little of it. An app's row shows its loudest stream. The output's meter moves only while something plays, and the input's only while an app records from it, so opening the mixer never wakes the microphone. The meters cost nothing while no mixer shows: Mochi asks the audio server for peak levels only while one is open, and the server sends 25 a second for each slider instead of the sound itself.

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
| `move <app> <device>` | Plays every stream of an app, or one stream by its id, through another output: its name, its description like `Headphones`, or `output` for the one in use |
| `meters <view> on\|off` | Runs the meters for 10 seconds, or stops them; the mixer sends this while it shows |

A `target` is `output` or `input` for the devices in use, a device's name, an app's name for all its streams, or a stream's id. App names match ignoring case when no app has the exact name. `pactl list short sinks`, `sources` and `sink-inputs` list them. `mochi ipc audio volume output +5` makes a volume key.

`max_volume = 200` lets the sliders and `volume` go to 200%; the OSD's volume bar then shows 200% as full, with a mark at 100%. Past 100%, the sliders and the bar turn to the accent color. Volume keys bound to `mochi ipc audio volume output +5` stop at the same place; `wpctl set-volume -l` has its own limit.

Double-click a slider to put it back to 100%. A volume keeps each device's balance: the loudest channel goes to the level and the others follow. While the mixer or another panel is open, the OSD's volume notice doesn't show, and doesn't come back late once it closes. `mochi ipc control-center open audio/mixer` opens the control center on the Sound page.

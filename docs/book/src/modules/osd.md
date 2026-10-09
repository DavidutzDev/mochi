# OSD

Shows changes as they happen, from any source: the output volume and mute, a switch of the output device, the microphone's mute, Caps Lock, Num Lock and the keyboard layout. It listens only and has no actions. The audio notices need PipeWire with its PulseAudio server, or PulseAudio.

## Keyboard layout

When the keyboard layout switches, the OSD shows the new layout's name, like "German". No Wayland protocol says the layout, so Mochi asks the compositor's IPC: Hyprland, niri and Sway tell it, and other compositors show nothing. At startup Mochi takes Hyprland's main keyboard, or on Sway the first keyboard with a named layout, then follows whichever keyboard switches. Keymaps without a name, like the ones apps such as `wtype` make to type, are skipped.

The OSD module also publishes the current layout for other views: `Daemon.state("osd").keyboard_layout` is the layout's name, or null while the compositor hasn't said. It's there with `layout = false` too, which only turns the notice off. `mochi status` shows it on the compositor line.

```toml
{{#include ../../../../modules/osd/settings.toml}}
```

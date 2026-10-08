# Privacy

A dot next to the island while an app records from a microphone or has a camera open: orange for the microphone, green for the camera. A muted microphone's dot is a ring, since whoever records hears nothing. A click on the dot mutes the default microphone, or unmutes it.

With `wide = true` in `[bubbles.privacy]`, the bubble names the apps:

```toml
[bubbles.privacy]
wide = true
```

The microphone comes from the [audio module](audio.md), which lists the apps recording from an input. It leaves out Mochi's own level meters and recordings of what an output plays, like a visualizer. Without the audio module, nothing shows for the microphone.

The camera comes from `/proc`: the processes with a `/dev/video*` device open, checked every 2 seconds. An app that uses the camera through PipeWire shows as `pipewire`, which opened the device for it.

```toml
{{#include ../../../../modules/privacy/settings.toml}}
```

`mochi ipc privacy status` prints who uses what.

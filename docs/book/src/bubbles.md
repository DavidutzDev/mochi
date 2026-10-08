# Bubbles

Bubbles are small round status items next to the island, like the music playing or missed notifications. The screen edge has five areas, from left to right: `left`, `center-left`, `center`, `center-right` and `right`. The island sits in one of them, `center` unless the [theme](theme.md) says otherwise.

Each module places its bubbles where it thinks best. These settings, in `config.toml`, move them:

```toml
{{#include ../../../crates/mochi-core/defaults/bubbles.toml}}
```

## Hidden bubbles

Past `max_per_area`, an area leaves out its least important bubbles and shows a "+N" pill with how many. Click it and the island lists them, each drawn as its module draws the bubble, with text when the module has a wide view. Click one to do what a click on the bubble does; the list closes. Escape, a click outside or a second click on "+N" closes it too. The list follows changes while it's open, and closes when the area has room for everything again.

## Stacking

With `stack = true`, each area shows one bubble instead of a row: the most important in front, and up to two more peeking out behind it, smaller, on the side away from the island. Hover the stack and it fans out into the row, each bubble clickable; it folds back a moment after the pointer leaves.

A bubble with news comes to the front for `news_ms`, 4 seconds by default, then goes back behind the most important one. News is a bubble appearing, or a change its module calls news: one more missed notification, another network, a device connecting, the battery turning critical, an app asking for attention in the tray, a reading turning critical. Changes that come all the time, like a timer or the CPU's use, never do. A module marks a showing as news with `BubbleSpec::news()`; a bubble's priority decides which is in front otherwise.

Resting the pointer on a bubble for `tooltip_ms`, 600 milliseconds by default, shows a tooltip beside it with more: the track and the artist, the network and its VPN, the battery's time left, who uses the microphone or the camera, how far a conversion is. `tooltip_ms = 0` turns them off. A module sets one with `BubbleSpec::tooltip(text)`, and a bubble's view can work one out from its payload with a `tooltip` property, which wins.

A stack holds every bubble of its area, so `max_per_area` doesn't apply to it.

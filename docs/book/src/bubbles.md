# Bubbles

Bubbles are small round status items next to the island, like the music playing or missed notifications. The screen edge has five areas, from left to right: `left`, `center-left`, `center`, `center-right` and `right`. The island sits in one of them, `center` unless the [theme](theme.md) says otherwise.

Each module places its bubbles where it thinks best. These settings, in `config.toml`, move them:

```toml
{{#include ../../../crates/mochi-core/defaults/bubbles.toml}}
```

## Stacking

With `stack = true`, each area shows one bubble instead of a row: the most important in front, and up to two more peeking out behind it, smaller, on the side away from the island. Hover the stack and it fans out into the row, each bubble clickable; it folds back a moment after the pointer leaves.

A bubble with news comes to the front for `news_ms`, 4 seconds by default, then goes back behind the most important one. News is a bubble appearing, or a change its module calls news: one more missed notification, another network, a device connecting, the battery turning critical, an app asking for attention in the tray, a reading turning critical. Changes that come all the time, like a timer or the CPU's use, never do. A module marks a showing as news with `BubbleSpec::news()`; a bubble's priority decides which is in front otherwise.

A stack holds every bubble of its area, so `max_per_area` doesn't apply to it.

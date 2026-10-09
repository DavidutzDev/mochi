# Media

Shows what's playing in any player that speaks MPRIS: Spotify, browsers, VLC, mpv with mpv-mpris, and most others. A new track takes the island with the cover, the progress and the controls; after a few seconds the music becomes a bubble with the cover and a progress ring. Click the bubble to bring the player back, and click or drag the progress line to seek. Once you click the player open, Escape or a click outside sends it back to its bubble.

Players that report a volume of their own, like Spotify, get a volume slider under the controls on the island, and next to the controls on the control center card when it has room. That volume is the player's, apart from the app's volume in the mixer: it's the one the player's own slider shows. Players without one, or that take no commands, get no slider. Double-click it to put it back to 100%.

When several players are open, the one that last started playing wins, and a playing one always beats a paused one. Arrows next to the player's name, on the island and on the control center card, switch to another player that has a track. The one you pick stays shown until it stops or closes, even when another starts a new track; then the rule above applies again.

The progress line waves while a track plays and lies flat when it pauses. `wavy = false` keeps it flat, and the theme's `waves = false` in `[motion]` keeps every wave in Mochi flat, whatever the modules say: see [Motion](../theme.md#motion). `line_color = "accent"` draws the line in the accent instead of the text's color. On the control center card and the desktop card the line only shows the progress; `card_seeks = true` makes it seek there too, where the player can.

```toml
{{#include ../../../../modules/media/settings.toml}}
```

| Action | What it does |
|---|---|
| `play-pause`, `play`, `pause` | Controls playback |
| `next`, `previous` | Changes track |
| `seek <seconds>` | Jumps to a position in the track |
| `volume <level>` | Sets the shown player's own volume: `40` sets it, `+5` and `-5` move it, up to 100 |
| `next-player`, `previous-player` | Shows another player, until it stops |
| `player <name>` | Shows the player with this name, like `spotify` or `firefox`, until it stops |

It offers the control center a Now Playing card, and the [desktop](widgets.md) a widget in three looks: the same card; artwork, a tall card with the cover big and the track, the progress and the controls under it; and cover, only the cover, without a card, with a button to play or pause in its corner.

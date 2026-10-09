# Media

Shows what's playing in any player that speaks MPRIS: Spotify, browsers, VLC, mpv with mpv-mpris, and most others. A new track takes the island with the cover, the progress and the controls; after a few seconds the music becomes a bubble with the cover and a progress ring. Click the bubble to bring the player back, and click or drag the progress bar to seek. Once you click the player open, Escape or a click outside sends it back to its bubble.

When several players are open, the one that last started playing wins, and a playing one always beats a paused one. Arrows next to the player's name, on the island and on the control center card, switch to another player that has a track. The one you pick stays shown until it stops or closes, even when another starts a new track; then the rule above applies again.

```toml
{{#include ../../../../modules/media/settings.toml}}
```

| Action | What it does |
|---|---|
| `play-pause`, `play`, `pause` | Controls playback |
| `next`, `previous` | Changes track |
| `seek <seconds>` | Jumps to a position in the track |
| `next-player`, `previous-player` | Shows another player, until it stops |
| `player <name>` | Shows the player with this name, like `spotify` or `firefox`, until it stops |

It offers the control center a Now Playing card.

# Media

Shows what's playing in any player that speaks MPRIS: Spotify, browsers, VLC, mpv with mpv-mpris, and most others. A new track takes the island with the cover, the progress and the controls; after a few seconds the music becomes a bubble with the cover and a progress ring. Click the bubble to bring the player back, and click or drag the progress bar to seek. Once you click the player open, Escape or a click outside sends it back to its bubble.

When several players are open, the one that last started playing wins, and a playing one always beats a paused one.

```toml
{{#include ../../../../modules/media/settings.toml}}
```

| Action | What it does |
|---|---|
| `play-pause`, `play`, `pause` | Controls playback |
| `next`, `previous` | Changes track |
| `seek <seconds>` | Jumps to a position in the track |

It offers the hub a Now Playing card.

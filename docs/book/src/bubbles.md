# Bubbles

Bubbles are small round status items next to the island, like the music playing or missed notifications. The screen edge has five areas, from left to right: `left`, `center-left`, `center`, `center-right` and `right`. The island sits in one of them, `center` unless the [theme](theme.md) says otherwise.

Each module places its bubbles where it thinks best. These settings, in `config.toml`, move them:

```toml
{{#include ../../../crates/mochi-core/defaults/bubbles.toml}}
```

# Theme

`theme.toml` sets how everything looks, for every module at once. Views only use these values, never their own, so a change here restyles the whole shell. `mochi reload` applies it at once.

The defaults are "Obsidian": a black island that disappears into the screen's bezel, graphite cards on it, and one accent color for what's active or important.

```toml
{{#include ../../../crates/mochi-core/defaults/theme.toml}}
```

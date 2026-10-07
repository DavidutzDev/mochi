# Theme

`theme.toml` sets how everything looks, for every module at once. Views only use these values, never their own, so a change here restyles the whole shell. `mochi reload` applies it at once.

The defaults are "Obsidian": a black island that disappears into the screen's bezel, graphite cards on it, and one accent color for what's active or important.

Text is set in Inter and icons in Material Symbols Rounded, which the Nix and Arch packages bring; `text.family` picks another font. Sizes follow one scale: four text sizes and one for big numbers, five spacings and three corner sizes, so every module lines up. `text.label` and `text.subtitle`, from before the scale, still work as `text.caption` and `text.body`, and `mochi config check` says so.

```toml
{{#include ../../../crates/mochi-core/defaults/theme.toml}}
```

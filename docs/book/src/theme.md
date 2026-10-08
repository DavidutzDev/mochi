# Theme

`theme.toml` sets how everything looks, for every module at once. Views only use these values, never their own, so a change here restyles the whole shell. `mochi reload` applies it at once.

The defaults are "Obsidian": a black island that disappears into the screen's bezel, graphite cards on it, and one accent color for what's active or important.

## Presets and light mode

`preset` picks a palette for every color role: `obsidian`, `catppuccin`, `nord`, `gruvbox`, `rose-pine` or `tokyo-night`. Each has a dark and a light version, which `appearance` picks: `"dark"`, `"light"`, or `"auto"` to follow the system's preference through the desktop portal, switching as it changes. What `[colors]` sets goes over the preset, so you can keep a preset and change only the accent.

`preset = "wallpaper"` makes the palette from your wallpaper: the accent from its most colorful hue, and the backgrounds from the same hue nearly grey, with lightness chosen so text stays readable. `wallpaper = "auto"` reads the image, or the plain color, that awww, swww or hyprpaper shows; a path reads that image. `mochi reload` reads it again after the wallpaper changes.

The settings panel lists the presets with their colors, and applies them as you pick.

## Motion

`[motion]` sets how things move. `reduced = true` turns animations off: views appear and change at once, and the island takes its shape without a spring. `speed` makes every animation faster or slower, 2 being twice as fast.

Text is set in Inter and icons in Material Symbols Rounded, which the Nix and Arch packages bring; `text.family` picks another font. Sizes follow one scale: four text sizes and one for big numbers, five spacings and three corner sizes, so every module lines up. `text.label` and `text.subtitle`, from before the scale, still work as `text.caption` and `text.body`, and `mochi config check` says so.

```toml
{{#include ../../../crates/mochi-core/defaults/theme.toml}}
```

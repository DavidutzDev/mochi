# Theme

`theme.toml` sets how everything looks, for every module at once. Views only use these values, never their own, so a change here restyles the whole shell. `mochi reload` applies it at once.

The defaults are "Obsidian": a black island that disappears into the screen's bezel, graphite cards on it, and one accent color for what's active or important.

## Presets and light mode

`preset` picks a theme, a color for every role: `obsidian`, `catppuccin`, `nord`, `gruvbox`, `rose-pine`, `tokyo-night`, or one you installed, see [Theme packages](#theme-packages). Each has a dark and a light version, which `appearance` picks: `"dark"`, `"light"`, or `"auto"` to follow the system's preference through the desktop portal, switching as it changes. What `[colors]` sets goes over the preset, so you can keep a preset and change only the accent.

`preset = "wallpaper"` makes the palette from your wallpaper: the accent from its most colorful hue, and the backgrounds from the same hue nearly grey, with lightness chosen so text stays readable. `wallpaper = "auto"` reads the image, or the plain color, that awww, swww or hyprpaper shows; a path reads that image. `mochi reload` reads it again after the wallpaper changes.

The settings panel lists the presets with their colors, and applies them as you pick. A preset picked there replaces the colors `[colors]` sets, in the file and in the panel, so you see that theme. Copy then gives the preset without them, for your own files.

## Theme packages

A theme is a directory with a `mochi-theme.toml`. Installed ones live in `~/.local/share/mochi/themes/<id>/` (`$XDG_DATA_HOME/mochi/themes`), and `preset = "<id>"` picks one like the themes Mochi brings, which are written the same way:

```toml
{{#include ../../../crates/mochi-core/themes/nord.toml}}
```

`[theme]` says what it is. `id` is lowercase letters, digits, `-` and `_`, starting with a letter, and must match the directory's name. `name` is what the settings panel shows, and `mochi` is the oldest Mochi it works with: a newer one is refused, and `mochi config check` says so. `description`, `authors` and `homepage` are optional.

`[dark]` and `[light]` give each role a color, the roles of `[colors]` below. A theme with only one of them uses it for both appearances, so a dark theme stays dark, and a role it leaves out takes Obsidian's of the same version, so a theme can be as short as an accent. Like every theme, it holds no views or code, so it can't run anything. A typo, like an unknown role or a color that isn't `#rrggbb` or `#aarrggbb`, is an error naming the key.

A theme can also set the rest of the look, with the sections `theme.toml` has below: `[text]` for its fonts and sizes, `[layout]` for the island's shape and spacing, and `[motion]` for how things move. What your `theme.toml` sets goes over them, and the settings panel shows the theme's values where you set nothing:

```toml
[text]
family = "IBM Plex Sans"

[motion]
speed = 1.5
```

They're checked like `theme.toml`, so a typo or a value out of range is an error naming its section. A font the theme names that isn't installed is drawn in another font Qt picks, so install the fonts a theme names.

`mochi bento add <directory or repository>` installs a theme, and `mochi bento try` tries it until you keep it or drop it: see [Bento](bento.md).

## Layout

`[layout] mode` picks the island's shape. `"island"` floats it `margin` away from the edge, rounded all around. `"notch"` attaches it to the edge, square there, with concave corners, the ears, flaring into the edge. `"bar"` draws a strip `idle_height` tall along the whole edge, like GNOME's or macOS's top bar: the bubbles sit on it without a background of their own and light up under the pointer, and the island sits in it, so the idle clock reads as part of the bar. A notice or a panel grows out of the bar like a notch, with ears where it leaves it. Windows keep out of the strip, and switching modes morphs from one to the other.

## Motion

`[motion]` sets how things move. `reduced = true` turns animations off: views appear and change at once, and the island takes its shape without a spring. `speed` makes every animation faster or slower, 2 being twice as fast.

Some progress lines and rings wave while something goes on: the media line while a track plays, the battery ring while it charges, the timer's ring while it runs, and the performance rings. `waves = false` draws every one of them flat. To keep some and not others, leave it on and turn off a module's own: `wavy = false` in `[module.media]`, `[module.battery]` or `[module.performance]`. With `reduced = true` the waves stay, but the media line's stops drifting.

Text is set in Inter and icons in Material Symbols Rounded, which the Nix and Arch packages bring; `text.family` picks another font. Sizes follow one scale: four text sizes and one for big numbers, five spacings and three corner sizes, so every module lines up. `text.label` and `text.subtitle`, from before the scale, still work as `text.caption` and `text.body`, and `mochi config check` says so.

```toml
{{#include ../../../crates/mochi-core/defaults/theme.toml}}
```

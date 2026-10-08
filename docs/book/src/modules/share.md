# Share

The picker that opens when an app asks to share your screen, and a bubble while something shares it.

When an app like Discord or OBS asks xdg-desktop-portal-hyprland for the screen, the island grows into a panel. It shows your screens and the windows the portal offers, each with a live picture, windows on other workspaces included. Click one, or move with the arrows and Tab and press Enter, to share it. **Region** (or R) lets you drag an area over the screen instead; Escape or a right click goes back. **Remember** lets the app keep the choice for next time. Escape, or a click outside, shares nothing.

## Switching what you share

The portal can't change what a running share captures, so by default Mochi shares a copy you can switch. The app gets a monitor of Mochi's own, `MOCHI-SHARE`, and Mochi draws a live copy of the screen, window or region you picked on it, pointer included. Click the sharing bubble, or run `mochi ipc share switch`, and the picker opens again: pick something else and the copy changes at once. The app keeps sharing the same monitor, so it never asks again, and the people watching see the new source.

`MOCHI-SHARE` is a Hyprland headless monitor, placed far from your screens so the pointer and your windows never reach it, at the size of your largest screen. It gets no island. A source with another shape is scaled to fit, with black around it. Mochi removes the monitor a few seconds after the app stops capturing it, or 30 seconds after the pick if the app never starts.

A restart of mochid, or a reload that restarts the share module, doesn't end the share. While the app captures `MOCHI-SHARE`, Mochi leaves the monitor in place, and the next start draws the copy on it again, with the same source and quality. The app sees the stream pause for the moment the copy is gone and carry on, without sharing again. The bubble comes back too.

With Switchable on, the picker also sets the quality the app receives: the frame rate (15, 30, 60, 90 or 120 fps) is the switchable monitor's refresh rate, and the resolution (Native, 480p, 720p, 1080p or 1440p) its size, scaled down from your largest screen with its shape kept. Each click steps to the next preset; `framerate` and `resolution` in the settings set how they start. The picker the bubble opens has them too, starting from the share's current quality, and a click there changes the running share at once: Mochi resizes its monitor, and the app gets a stream of the new size or pace, as when a shared window is resized. The app can still send less, like Discord without Nitro. A share that isn't switchable leaves the quality to the app.

Turn off **Switchable** in the picker to share the choice itself for one share, or set `switchable = false` for every share. Without the switchable copy, **Remember** lets the app keep the choice, which a monitor that only exists while it's shared can't offer. Switching needs Hyprland; elsewhere the picker shares the choice itself.

```toml
{{#include ../../../../modules/share/settings.toml}}
```

## The bubble

While the screen is shared, a screen icon breathes next to the island. It shows once a capture has lasted a second and a half, so a screenshot's one-frame capture doesn't light it up, and never for the picker's own previews.

Hyprland only reports a capture when it starts or stops, so the share module writes down what's captured in its session directory, and gives it back to the compositor when mochid starts again within two minutes. A share that started before a restart keeps its bubble, and the bubble goes when that share stops.

## Setup

The portal runs one program as its picker. The Nix package has it as `mochi-share-picker`, and the home-manager module points the portal at it in `~/.config/hypr/xdph.conf` (`programs.mochi.portalPicker.enable`, on by default). The Arch packages install it as `/usr/bin/mochi-share-picker`. Elsewhere, write that file yourself, with a small script that runs `mochi share-pick "$@"`:

```
screencopy {
    custom_picker_binary = /path/to/mochi-share-picker
}
```

The portal reads the file when it starts: restart it with `systemctl --user restart xdg-desktop-portal-hyprland`, or log in again.

When Mochi isn't running, or the share module isn't enabled, `mochi share-pick` opens the portal's own picker, so sharing never breaks.

The bubble needs Hyprland, which reports captures on its event socket.

## Actions

| Action | What it does |
|---|---|
| `pick <remember> [windows]` | Asks what to share and prints the portal's answer; `mochi share-pick` runs it |
| `switch` | Opens the picker to change what a switchable share copies; clicking the bubble does this |
| `cancel` | Shares nothing |

The picker sends `screen`, `window`, `region`, `back`, `area`, `remember` and `switchable` itself.

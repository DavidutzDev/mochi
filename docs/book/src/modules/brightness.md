# Brightness

The laptop's backlight, the keyboard's backlight, and external monitors over DDC/CI. Without any of them, it shows nothing.

The backlight comes from `/sys/class/backlight`, and the keyboard's from the LED named like `tpacpi::kbd_backlight` in `/sys/class/leds`. Mochi changes both through logind, which lets the user of the active session set them without a udev rule, and writes the file itself when logind refuses. Mochi reads them twice a second, so a change made elsewhere, like a brightness key the firmware handles or Fn+Space on a ThinkPad, shows the OSD too.

Monitors come from [ddcutil](https://www.ddcutil.com). It needs the `i2c-dev` kernel module and access to `/dev/i2c-*`, which the `i2c` group or ddcutil's udev rule gives. On NixOS, `hardware.i2c.enable = true` does both and adds your user to the group once you list it in `users.users.<name>.extraGroups`. Finding the monitors takes a few seconds, so Mochi does it once at the start; `mochi ipc brightness refresh` looks again after you plug one in. While a slider moves, each monitor gets only the latest level, as DDC/CI is slow.

The control center has a card with a slider for each display and one for the keyboard. The OSD shows the level when it changes, and scrolling on it changes it.

```toml
{{#include ../../../../modules/brightness/settings.toml}}
```

## Keybinds

```sh
mochi ipc brightness up              # every display, by the step
mochi ipc brightness down backlight  # only the laptop's screen
mochi ipc brightness set 40 DP-1     # a monitor, by its output or model
mochi ipc brightness set +10 external
mochi ipc brightness up keyboard     # the keyboard's backlight, a level up
mochi ipc brightness status
```

`all`, the default, means every display; the keyboard changes only when you name it. A keyboard often has two or three levels, so `up` and `down` move it at least one level, whatever the step.

On Hyprland:

```ini
bindel = , XF86MonBrightnessUp, exec, mochi ipc brightness up
bindel = , XF86MonBrightnessDown, exec, mochi ipc brightness down
bindel = , XF86KbdBrightnessUp, exec, mochi ipc brightness up keyboard
bindel = , XF86KbdBrightnessDown, exec, mochi ipc brightness down keyboard
```

`mochi doctor` says when ddcutil is missing.

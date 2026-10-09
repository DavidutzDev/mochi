# Brightness

The laptop's backlight and external monitors over DDC/CI. Without either, it shows nothing.

The backlight comes from `/sys/class/backlight`. Mochi changes it through logind, which lets the user of the active session set it without a udev rule, and writes the file itself when logind refuses. Mochi reads it twice a second, so a change made elsewhere, like a brightness key the firmware handles, shows the OSD too.

Monitors come from [ddcutil](https://www.ddcutil.com). It needs the `i2c-dev` kernel module and access to `/dev/i2c-*`, which the `i2c` group or ddcutil's udev rule gives. On NixOS, `hardware.i2c.enable = true` does both and adds your user to the group once you list it in `users.users.<name>.extraGroups`. Finding the monitors takes a few seconds, so Mochi does it once at the start; `mochi ipc brightness refresh` looks again after you plug one in. While a slider moves, each monitor gets only the latest level, as DDC/CI is slow.

The control center has a card with a slider for each display. The OSD shows the level when it changes, and scrolling on it changes it.

```toml
{{#include ../../../../modules/brightness/settings.toml}}
```

## Keybinds

```sh
mochi ipc brightness up              # every display, by the step
mochi ipc brightness down backlight  # only the laptop's screen
mochi ipc brightness set 40 DP-1     # a monitor, by its output or model
mochi ipc brightness set +10 external
mochi ipc brightness status
```

On Hyprland:

```ini
bindel = , XF86MonBrightnessUp, exec, mochi ipc brightness up
bindel = , XF86MonBrightnessDown, exec, mochi ipc brightness down
```

`mochi doctor` says when ddcutil is missing.

# Night light

Warmer screens in the evening. Mochi sets each output's gamma itself through `wlr-gamma-control-unstable-v1`, which Hyprland, Sway and niri have, so no other program runs. Turn off other night lights, like wlsunset, gammastep or hyprsunset: a compositor gives one program each output's gamma, and Mochi says so when it can't have it. When mochid stops, even by crashing, the screens go back to normal.

It turns on by hand, from the control center's tile or `mochi ipc nightlight toggle`, or on a schedule: between two times of day, or from sunset to sunrise where you are, worked out from your latitude and longitude. A schedule warms up and cools down over `fade_minutes`. Turning it on or off by hand slides there in under a second, and lasts until the schedule changes next; `auto` hands it back to the schedule at once.

```toml
{{#include ../../../../modules/nightlight/settings.toml}}
```

For sunset to sunrise in Paris:

```toml
[module.nightlight]
schedule = "sun"
latitude = 48.85
longitude = 2.35
```

| Action | What it does |
|---|---|
| `on`, `off`, `toggle` | Warm or daylight until the schedule changes next |
| `auto` | Follows the schedule again |
| `temperature <kelvin>` | The night's temperature until mochid restarts, from 1000 to 6000 |
| `status` | Prints whether it's on, how warm, and any problem |

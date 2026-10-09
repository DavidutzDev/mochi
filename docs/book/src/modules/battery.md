# Battery

A laptop's battery, from UPower over D-Bus. Without a battery, as on a desktop, it shows nothing.

On battery, dropping past a level shows a short notice on the island, like "Battery at 50% · 2 h 20 min left", once per discharge: at 80, 50, 20 and 10% by default. At or under the warning level, 50% by default, a bubble with the level stays next to the island until you plug in. At the critical level, 10% by default, the bubble turns red and breathes, and the notice turns red and stays ten seconds, even over an open panel. Plugging the charger in or out shows a notice too, with the time left on battery.

The control center has a card with the level and the time until empty or full. Clicking the bubble opens the control center.

```toml
{{#include ../../../../modules/battery/settings.toml}}
```

`notices = []` turns the level notices off, and `warning = 0` the bubble. The levels count the combined charge of every battery, as UPower's display device reports it.

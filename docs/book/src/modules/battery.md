# Battery

A laptop's batteries, and the batteries of mice, keyboards, controllers, headsets and other devices, from UPower over D-Bus. Without any of them, as on a desktop without wireless devices, it shows nothing.

On battery, dropping past a level shows a short notice on the island, like "Battery at 50% · 2 h 20 min left", once per discharge: at 80, 50, 20 and 10% by default. At or under the warning level, 50% by default, a bubble with the level stays next to the island until you plug in. At the critical level, 10% by default, the bubble turns red and breathes, and the notice turns red and stays ten seconds, even over an open panel. Plugging the charger in or out shows a notice too, with the time left on battery.

The control center has a card with the level and the time until empty or full. Under it, a line for each battery of a laptop with two, and one for each device that reports its battery to UPower, with its name and level, red when low. When a device drops to 15%, the island says so once, like "MX Master 3 battery low · 15%", and again only after it charges. A device already low when Mochi starts says nothing; one that connects already low does. Clicking the bubble opens the control center.

```toml
{{#include ../../../../modules/battery/settings.toml}}
```

`notices = []` turns the level notices off, `warning = 0` the bubble, and `peripherals = 0` the devices' notices. The levels count the combined charge of every battery, as UPower's display device reports it.

`mochi ipc battery status` prints each battery and device with its level.

On the [desktop](widgets.md), the card is a widget too, and so is a ring: the level as a ring with the percent inside, and under it "Charging", "Fully charged", "Plugged in" or "On battery", with how long until full or empty when UPower knows. The ring waves while the battery charges, and turns red when it's low.

## Bluetooth devices

UPower lists Bluetooth devices that report a battery too, so a Bluetooth mouse shows on this card and on the [Bluetooth](bluetooth.md) page. The card is the one list of every battery; the Bluetooth page shows the level next to the device you connect or forget. Only this module says when a battery gets low, so a device gets one notice. When UPower finds one device twice, through the kernel and through BlueZ, the card lists it once.

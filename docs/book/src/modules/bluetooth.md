# Bluetooth

Power, paired devices with their battery, scanning and pairing, from BlueZ over D-Bus.

While a device is connected, a bubble shows the Bluetooth symbol and the device's battery when it reports one, red at 15% or less. Click it to open the hub's Bluetooth page. When a paired device connects or disconnects, the island shows a short notice, like "Buds connected · 70%".

The **Bluetooth page** has the power switch, the paired devices, connected first, to connect, disconnect or forget, and the devices in range to pair. **Scan** looks for 30 seconds. Pairing trusts the device, so it can connect by itself later, and connects it. The **home card** is a tile that turns Bluetooth on and off.

Mochi is BlueZ's pairing agent, so pairing asks on the island: whether the code a phone shows matches, whether a device without a code may pair, the PIN an older device expects, or a code to type on a keyboard. Escape or a click outside says no.

```toml
{{#include ../../../../modules/bluetooth/settings.toml}}
```

| Action | What it does |
|---|---|
| `power [on\|off\|toggle]` | Turns Bluetooth on or off |
| `powered` | Succeeds when Bluetooth is on, for scripts |
| `scan [on\|off\|toggle]` | Looks for devices to pair |
| `connect <device>`, `disconnect <device>` | By name or address |
| `pair <device>` | Pairs a device in range, trusts it and connects it |
| `forget <device>` | Forgets a paired device |

The pairing questions send `answer yes|no` and `pin <code>` themselves.

Without an adapter, or with BlueZ stopped, the page and the tile say so, and the module waits for BlueZ to start.

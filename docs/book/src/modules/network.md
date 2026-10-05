# Network

Wi-Fi, Ethernet, VPNs and airplane mode, from NetworkManager over D-Bus.

A bubble shows the connection: the Wi-Fi's strength, Ethernet, or offline, with a small lock while a VPN runs. Click it to open the hub's Network page. When you connect, disconnect, or a VPN starts or stops, the island shows a short notice.

The **Network page** has the Wi-Fi and airplane switches, the Wi-Fi networks in range with the one in use first, the wired devices and the VPNs. Click a network to join it. A saved one connects at once; a new secured one asks for its password on the island, and Enter joins. If the network doesn't come up within 30 seconds, Mochi forgets it, so a wrong password isn't kept, and says so. The trash button forgets a saved network. Networks that ask for a user name (WPA Enterprise) need setting up once with `nmcli` or `nm-connection-editor`; after that they join from the page like any saved one.

The **home card** has tiles for Wi-Fi (Ethernet on machines without it), the VPN when you have one, and airplane mode. Airplane mode turns Wi-Fi, the mobile radio and Bluetooth off, and turns back on the ones that were on.

```toml
{{#include ../../../../modules/network/settings.toml}}
```

| Action | What it does |
|---|---|
| `wifi [on\|off\|toggle]` | Turns Wi-Fi on or off |
| `airplane [on\|off\|toggle]` | Turns every radio off, or back on |
| `scan` | Looks for Wi-Fi networks |
| `connect <name>` | Joins a Wi-Fi network or starts a saved connection; a new secured network asks for its password |
| `disconnect [name]` | Leaves a network, stops a VPN, or disconnects a wired device by its interface; the Wi-Fi when left out |
| `forget <name>` | Forgets a saved Wi-Fi network |
| `vpn <name> [on\|off\|toggle]` | Starts or stops a VPN |

The password prompt sends `password <ssid> <password>` itself. Mochi never logs it.

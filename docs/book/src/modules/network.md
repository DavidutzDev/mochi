# Network

Wi-Fi, Ethernet, VPNs and airplane mode, from NetworkManager over D-Bus.

A bubble shows the connection: the Wi-Fi's strength, Ethernet, or offline, with a small lock while a VPN runs. Click it to open the control center's Network page. When you connect, disconnect, or a VPN starts or stops, the island shows a short notice.

The **Network page** has the Wi-Fi and airplane switches, the Wi-Fi networks in range with the one in use first, the wired devices and the VPNs. Click a network to join it. A saved one connects at once; a new secured one asks for its password on the island, and Enter joins. If the network doesn't come up within 30 seconds, Mochi forgets it, so a wrong password isn't kept, and says so. The trash button forgets a saved network. A network that asks for a user name, WPA Enterprise like eduroam, asks for it with the password and joins with PEAP and MSCHAPv2, what most of them use; one that needs another method or a certificate still needs setting up once with `nmcli` or `nm-connection-editor`. **Hidden…** next to Scan joins a network that doesn't say its name: type it, pick Open, Password or Enterprise, and what that asks.

Mochi is NetworkManager's secret agent too. When a connection needs a password NetworkManager doesn't have, like a saved network whose password changed or a connection started with `nmcli`, it asks on the island, saying when the last one didn't work. Cancel tells NetworkManager so. It answers for Wi-Fi passwords and 802.1X passwords; a VPN's secrets go to another agent, like the VPN plugin's own.

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
| `hidden` | Asks on the island for a hidden network's name and security, then joins it |
| `disconnect [name]` | Leaves a network, stops a VPN, or disconnects a wired device by its interface; the Wi-Fi when left out |
| `forget <name>` | Forgets a saved Wi-Fi network |
| `vpn <name> [on\|off\|toggle]` | Starts or stops a VPN |

The prompt sends `answer <json>` itself, with the password and what else it asked; `password <ssid> <password>` joins a network in range from a script. Mochi never logs either.

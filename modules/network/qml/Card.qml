import QtQuick
import qs.island

// The hub's home card: Wi-Fi (or Ethernet without it), the VPN and airplane
// mode, as tiles. Wi-Fi and airplane mode switch on a click; the others
// open the Network page.
Item {
    id: root

    property var payload: null
    readonly property bool available: payload?.available ?? false
    readonly property bool hidden: !available
    readonly property bool wifi: payload?.wifi?.available ?? false
    readonly property var vpns: payload?.vpns ?? []
    readonly property var activeVpn: vpns.find(vpn => vpn.active) ?? null
    readonly property var wired: (payload?.wired ?? []).find(device => device.connected) ?? null

    implicitHeight: available ? 64 : 40

    function page(): void {
        Daemon.command("hub", "open", ["network/page"]);
    }

    Text {
        anchors.centerIn: parent
        visible: !root.available
        text: "NetworkManager isn't running"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Row {
        id: tiles

        visible: root.available
        width: parent.width
        spacing: 8

        readonly property int count: 1 + (root.vpns.length > 0 ? 1 : 0) + 1
        readonly property real tileWidth: (width - spacing * (count - 1)) / count

        Tile {
            width: tiles.tileWidth
            vertical: false
            icon: root.wifi ? (root.payload?.wifi?.enabled ? (root.payload?.status?.icon ?? "wifi") : "wifi-off") : "ethernet"
            title: root.wifi ? "Wi-Fi" : "Ethernet"
            subtitle: {
                if (root.wifi) {
                    if (!root.payload.wifi.enabled)
                        return "Off";
                    const connected = (root.payload.networks ?? []).find(network => network.connected);
                    return connected ? connected.ssid : "Not connected";
                }
                return root.wired ? (root.wired.connection ?? root.wired.interface) : "Unplugged";
            }
            checked: root.wifi ? (root.payload?.wifi?.enabled ?? false) : root.wired !== null
            onClicked: {
                if (root.wifi)
                    Daemon.command("network", "wifi", ["toggle"]);
                else
                    root.page();
            }
        }

        Tile {
            visible: root.vpns.length > 0
            width: tiles.tileWidth
            vertical: false
            icon: "lock"
            title: "VPN"
            subtitle: root.activeVpn ? root.activeVpn.id : root.vpns.length === 1 ? root.vpns[0].id : "Off"
            checked: root.activeVpn !== null
            onClicked: {
                if (root.vpns.length === 1)
                    Daemon.command("network", "vpn", [root.vpns[0].id, "toggle"]);
                else
                    root.page();
            }
        }

        Tile {
            width: tiles.tileWidth
            vertical: false
            icon: "airplane"
            title: "Airplane"
            subtitle: root.payload?.airplane ? "On" : "Off"
            checked: root.payload?.airplane ?? false
            onClicked: Daemon.command("network", "airplane", ["toggle"])
        }
    }
}

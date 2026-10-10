import QtQuick
import qs.island

// The control center's Network page: the connection, Wi-Fi and airplane
// switches, the Wi-Fi networks in range, the wired devices and the VPNs. A
// click on a network joins it; a new secured one asks for its password on the
// island.
Item {
    id: root

    property var payload: null
    readonly property bool available: payload?.available ?? false
    readonly property bool wifi: payload?.wifi?.available ?? false
    readonly property bool wifiOn: payload?.wifi?.enabled ?? false
    readonly property var networks: payload?.networks ?? []
    readonly property var wired: payload?.wired ?? []
    readonly property var vpns: payload?.vpns ?? []
    // Up to six networks show; more scroll.
    readonly property int rows: Math.min(networks.length, 6)

    implicitHeight: available ? column.implicitHeight : 60

    // A fresh list each time the page opens.
    Component.onCompleted: Daemon.command("network", "scan", [])

    Text {
        anchors.centerIn: parent
        visible: !root.available
        text: "NetworkManager isn't running"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Column {
        id: column

        visible: root.available
        width: parent.width
        spacing: Theme.spaceSmall

        // The connection, and the switches.
        PanelHeader {
            width: parent.width
            title: {
                const status = root.payload?.status;
                if (!status)
                    return "";
                return status.vpn ? `${status.label} · ${status.vpn}` : status.label;
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.wifi
                text: "Wi-Fi"
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            Switch {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.wifi
                checked: root.wifiOn
                onToggled: Daemon.command("network", "wifi", ["toggle"])
            }

            Item {
                width: Theme.spaceSmall
                height: 1
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Airplane"
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            Switch {
                anchors.verticalCenter: parent.verticalCenter
                checked: root.payload?.airplane ?? false
                onToggled: Daemon.command("network", "airplane", ["toggle"])
            }
        }

        // Wi-Fi.
        Item {
            visible: root.wifi
            width: parent.width
            height: 26

            SectionLabel {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: "Wi-Fi networks"
            }

            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                visible: root.wifiOn

                // A network that doesn't say its name, asked on the island.
                Button {
                    tone: "ghost"
                    text: "Hidden…"
                    onClicked: Daemon.command("network", "hidden", [])
                }

                Button {
                    tone: "ghost"
                    text: "Scan"
                    onClicked: Daemon.command("network", "scan", [])
                }
            }
        }

        Text {
            visible: root.wifi && (!root.wifiOn || root.networks.length === 0)
            text: root.wifiOn ? "Looking for networks…" : "Wi-Fi is off"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        ListView {
            id: list

            visible: root.wifi && root.wifiOn && root.networks.length > 0
            width: parent.width
            height: root.rows * 52 + Math.max(root.rows - 1, 0) * spacing
            clip: true
            spacing: Theme.spaceSmall
            boundsBehavior: Flickable.StopAtBounds
            // A count, not the array: a new array would rebuild every row.
            model: root.wifiOn ? root.networks.length : 0

            ScrollFade {
                view: list
            }

            WheelScroll {
                view: list
            }

            delegate: ListRow {
                id: row

                required property int index
                readonly property var network: root.networks[index] ?? {}

                width: list.width
                height: 52
                leadingSize: 22
                icon: network.icon ?? "wifi"
                title: network.ssid ?? ""
                subtitle: {
                    if (network.connected)
                        return "Connected";
                    const parts = [];
                    if (network.saved)
                        parts.push("Saved");
                    parts.push(network.enterprise ? "Enterprise" : network.secure ? "Secured" : "Open");
                    return parts.join(" · ");
                }
                selected: network.connected ?? false
                marker: true
                onClicked: {
                    if (!row.network.connected)
                        Daemon.command("network", "connect", [row.network.ssid]);
                }

                trailing: [
                    Symbol {
                        anchors.verticalCenter: parent?.verticalCenter
                        visible: row.network.secure ?? false
                        name: "lock"
                        size: 12
                        color: Theme.muted
                    },
                    Button {
                        visible: row.network.connected ?? false
                        tone: "ghost"
                        text: "Disconnect"
                        onClicked: Daemon.command("network", "disconnect", [row.network.ssid])
                    },
                    IconButton {
                        visible: (row.network.saved ?? false) && !(row.network.connected ?? false)
                        icon: "trash"
                        size: 14
                        tone: "neutral"
                        onClicked: Daemon.command("network", "forget", [row.network.ssid])
                    }
                ]
            }
        }

        // Wired.
        SectionLabel {
            visible: root.wired.length > 0
            topPadding: 4
            text: "Wired"
        }

        Repeater {
            model: root.wired.length

            ListRow {
                id: wiredRow

                required property int index
                readonly property var device: root.wired[index] ?? {}

                width: column.width
                height: 52
                leadingSize: 22
                icon: "ethernet"
                title: device.connection ?? device.interface ?? ""
                subtitle: device.connected ? `Connected · ${device.interface}` : `Not connected · ${device.interface}`
                trailing: [
                    Button {
                        visible: wiredRow.device.connected ?? false
                        tone: "ghost"
                        text: "Disconnect"
                        onClicked: Daemon.command("network", "disconnect", [wiredRow.device.interface])
                    }
                ]
            }
        }

        // VPNs.
        SectionLabel {
            visible: root.vpns.length > 0
            topPadding: 4
            text: "VPN"
        }

        Repeater {
            model: root.vpns.length

            SwitchRow {
                id: vpnRow

                required property int index
                readonly property var vpn: root.vpns[index] ?? {}

                width: column.width
                height: 52
                flat: false
                leadingSize: 22
                icon: "vpn_lock"
                title: vpn.id ?? ""
                subtitle: vpn.active ? "On" : vpn.connecting ? "Connecting…" : "Off"
                checked: vpn.active ?? false
                onToggled: Daemon.command("network", "vpn", [vpnRow.vpn.id, "toggle"])
            }
        }
    }
}

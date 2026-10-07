import QtQuick
import qs.island

// The hub's Bluetooth page: the power switch, the paired devices to
// connect, disconnect or forget, and a scan for devices in range to pair.
Item {
    id: root

    property var payload: null
    readonly property bool available: payload?.available ?? false
    readonly property bool powered: payload?.powered ?? false
    readonly property var paired: payload?.paired ?? []
    readonly property var found: payload?.found ?? []

    implicitHeight: available ? column.implicitHeight : 60

    Text {
        anchors.centerIn: parent
        visible: !root.available
        text: "No Bluetooth adapter, or BlueZ isn't running"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Column {
        id: column

        visible: root.available
        width: parent.width
        spacing: Theme.spaceSmall

        PanelHeader {
            width: parent.width
            title: root.powered ? "Bluetooth is on" : "Bluetooth is off"

            Switch {
                anchors.verticalCenter: parent.verticalCenter
                checked: root.powered
                onToggled: Daemon.command("bluetooth", "power", ["toggle"])
            }
        }

        SectionLabel {
            visible: root.powered
            topPadding: 4
            text: "Paired devices"
        }

        Text {
            visible: root.powered && root.paired.length === 0
            text: "Nothing paired yet"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        // Counts, not the arrays: a new array would rebuild every row.
        Repeater {
            model: root.powered ? root.paired.length : 0

            ListRow {
                id: pairedRow

                required property int index
                readonly property var device: root.paired[index] ?? {}

                width: column.width
                height: 52
                leadingSize: 22
                selected: device.connected ?? false
                marker: true
                title: device.name ?? ""
                subtitle: {
                    const parts = [device.connected ? "Connected" : "Not connected"];
                    if (device.battery !== null && device.battery !== undefined)
                        parts.push(`${device.battery}%`);
                    return parts.join(" · ");
                }
                onClicked: Daemon.command("bluetooth", device.connected ? "disconnect" : "connect", [device.address])

                leading: DeviceIcon {
                    anchors.centerIn: parent
                    icon: pairedRow.device.icon ?? ""
                }

                trailing: [
                    Button {
                        tone: "ghost"
                        text: pairedRow.device.connected ? "Disconnect" : "Connect"
                        onClicked: Daemon.command("bluetooth", pairedRow.device.connected ? "disconnect" : "connect", [pairedRow.device.address])
                    },
                    IconButton {
                        icon: "trash"
                        size: 14
                        tone: "neutral"
                        onClicked: Daemon.command("bluetooth", "forget", [pairedRow.device.address])
                    }
                ]
            }
        }

        Item {
            visible: root.powered
            width: parent.width
            height: 30

            SectionLabel {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: "Devices in range"
            }

            Button {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                tone: root.payload?.scanning ? "accent" : "ghost"
                text: root.payload?.scanning ? "Scanning…" : "Scan"
                onClicked: Daemon.command("bluetooth", "scan", ["toggle"])
            }
        }

        Text {
            visible: root.powered && root.found.length === 0
            text: root.payload?.scanning ? "Looking for devices…" : "Scan to find devices to pair"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Repeater {
            model: root.powered ? root.found.length : 0

            ListRow {
                id: foundRow

                required property int index
                readonly property var device: root.found[index] ?? {}

                width: column.width
                height: 52
                leadingSize: 22
                title: device.name ?? ""
                subtitle: device.address ?? ""
                onClicked: Daemon.command("bluetooth", "pair", [device.address])

                leading: DeviceIcon {
                    anchors.centerIn: parent
                    icon: foundRow.device.icon ?? ""
                }

                trailing: [
                    Button {
                        tone: "ghost"
                        text: "Pair"
                        onClicked: Daemon.command("bluetooth", "pair", [foundRow.device.address])
                    }
                ]
            }
        }
    }
}

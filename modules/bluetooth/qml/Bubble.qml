import QtQuick
import qs.island

// While a device is connected: the Bluetooth symbol, with the device's
// battery when it reports one. A click opens the hub's Bluetooth page.
Item {
    id: root

    property var payload: ({})
    readonly property var battery: payload.battery ?? null

    implicitWidth: row.implicitWidth + 8
    implicitHeight: 26

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceTiny

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: "bluetooth_connected"
            size: 15
            color: Theme.accent
        }

        RollingText {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.battery !== null
            text: `${root.battery}%`
            color: root.battery !== null && root.battery <= 15 ? Theme.danger : Theme.foreground
            pixelSize: Theme.textCaption
        }
    }
}

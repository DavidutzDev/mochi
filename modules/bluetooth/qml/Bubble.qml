import QtQuick
import qs.island

// While a device is connected: the Bluetooth symbol, with the device's
// battery when it reports one. A click opens the hub's Bluetooth page.
Item {
    id: root

    property var payload: ({})
    readonly property var battery: payload.battery ?? null
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: {
        if ((payload.count ?? 1) > 1)
            return `${payload.count} devices connected`;
        const name = payload.name ?? "A device";
        return payload.battery != null ? `${name} · ${payload.battery}% battery` : `${name} connected`;
    }

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

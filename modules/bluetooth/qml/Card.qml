import QtQuick
import qs.island

// The hub's home tile: Bluetooth on or off, with what's connected. A click
// switches it.
Item {
    id: root

    property var payload: null
    readonly property bool available: payload?.available ?? false
    readonly property var connected: payload?.connected ?? []
    // No adapter: nothing worth a card.
    readonly property bool hidden: !available

    implicitHeight: 64

    Tile {
        anchors.fill: parent
        vertical: false
        enabled: root.available
        opacity: root.available ? 1 : 0.5
        icon: "bluetooth"
        title: "Bluetooth"
        subtitle: {
            if (!root.available)
                return "No adapter";
            if (!root.payload.powered)
                return "Off";
            if (root.connected.length === 1)
                return root.connected[0];
            if (root.connected.length > 1)
                return `${root.connected.length} devices`;
            return "On";
        }
        checked: root.payload?.powered ?? false
        onClicked: Daemon.command("bluetooth", "power", ["toggle"])
    }
}

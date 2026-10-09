import QtQuick
import qs.island

// The control center's tile: night light on or off, and how warm or what turns
// it on. A click turns it on or off until the schedule changes next.
Item {
    id: root

    property var payload: null
    readonly property bool hidden: !(payload?.available ?? false)
    readonly property bool on: payload?.on ?? false

    implicitHeight: Theme.rowHeight

    Tile {
        anchors.fill: parent
        vertical: false
        icon: "nightlight"
        title: "Night light"
        checked: root.on && !root.payload?.problem
        subtitle: {
            if (root.payload?.problem)
                return "Unavailable";
            if (root.on)
                return `${root.payload.current} K`;
            const schedule = root.payload?.schedule ?? "manual";
            return schedule === "sun" ? "At sunset" : schedule === "times" ? "On a schedule" : "Off";
        }
        onClicked: Daemon.command("nightlight", "toggle", [])
    }
}

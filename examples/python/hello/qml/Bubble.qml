import QtQuick
import qs.island

// How many times it said something.
Item {
    id: root

    property var payload: ({})

    implicitWidth: Math.max(26, count.implicitWidth + 12)
    implicitHeight: 26

    RollingText {
        id: count

        anchors.centerIn: parent
        text: String(root.payload.said ?? 0)
        color: Theme.accent
        pixelSize: Theme.textCaption
        weight: Theme.weightTitle
    }
}

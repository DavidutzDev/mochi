import QtQuick
import qs.island

// How many times it said something.
Item {
    id: root

    property var payload: ({})

    implicitWidth: Math.max(26, count.implicitWidth + 12)
    implicitHeight: 26

    Text {
        id: count

        anchors.centerIn: parent
        text: root.payload.said ?? 0
        color: Theme.accent
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
        font.weight: Font.DemiBold
    }
}

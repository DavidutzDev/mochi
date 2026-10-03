import QtQuick
import qs.island

// The bubble for missed notifications: a bell with their count.
Item {
    id: root

    property var payload: ({})

    implicitWidth: 26
    implicitHeight: 26

    Glyph {
        anchors.centerIn: parent
        name: "bell"
        size: 18
    }

    Rectangle {
        anchors.right: parent.right
        anchors.top: parent.top
        width: Math.max(14, count.implicitWidth + 6)
        height: 14
        radius: 7
        color: Theme.accent

        Text {
            id: count

            anchors.centerIn: parent
            text: root.payload.count > 99 ? "99+" : String(root.payload.count ?? 0)
            color: Theme.background
            font.pixelSize: 9
            font.weight: Font.Bold
        }
    }
}

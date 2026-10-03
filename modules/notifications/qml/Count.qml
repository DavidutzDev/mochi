import QtQuick
import qs.island

// The bubble for missed notifications: a bell with their count.
Item {
    id: root

    property var payload: ({})

    implicitWidth: 26
    implicitHeight: 26

    Symbol {
        anchors.centerIn: parent
        name: "bell"
        size: 18
    }

    Badge {
        anchors.right: parent.right
        anchors.top: parent.top
        count: root.payload.count ?? 0
    }
}

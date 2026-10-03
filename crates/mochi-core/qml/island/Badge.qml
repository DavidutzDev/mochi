import QtQuick

// A count in an accent circle, like missed notifications.
Rectangle {
    id: root

    property int count: 0

    implicitWidth: Math.max(implicitHeight, label.implicitWidth + 7)
    implicitHeight: 15
    radius: height / 2
    color: Theme.accent

    Text {
        id: label

        anchors.centerIn: parent
        text: root.count > 99 ? "99+" : String(root.count)
        color: Theme.onAccent
        font.pixelSize: 9
        font.family: Theme.fontFamily
        font.weight: Font.Bold
    }
}

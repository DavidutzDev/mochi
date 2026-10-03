import QtQuick
import qs.island

// A button for a notification action or a history command.
Rectangle {
    id: root

    property string text: ""
    property string icon: ""
    signal clicked

    implicitWidth: icon ? 26 : label.implicitWidth + 24
    implicitHeight: 26
    radius: height / 2
    color: area.containsMouse ? Qt.lighter(Theme.surface, 1.6) : Theme.surface

    Text {
        id: label

        anchors.centerIn: parent
        visible: root.icon === ""
        text: root.text
        color: Theme.foreground
        font.pixelSize: 12
        font.weight: Font.Medium
    }

    Glyph {
        anchors.centerIn: parent
        visible: root.icon !== ""
        name: root.icon || "bell"
        size: 14
        color: Theme.muted
    }

    MouseArea {
        id: area

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}

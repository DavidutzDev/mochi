import QtQuick
import qs.island

// The island while an area to share is drawn: a way back to the list, and
// what to do.
Item {
    id: root

    property var payload: ({})

    implicitWidth: row.implicitWidth + 12
    implicitHeight: row.implicitHeight + 8

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 8

        Button {
            anchors.verticalCenter: parent.verticalCenter
            tone: "ghost"
            icon: "window"
            text: "Back"
            onClicked: Daemon.command("share", "back", [])
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: 18
            color: Theme.raised
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: 8
            text: "Drag the area to share"
            color: Theme.muted
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
        }
    }
}

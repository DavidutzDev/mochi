import QtQuick
import qs.island

// A compact status, like "headphones connected".
Item {
    property var payload: ({})

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 10

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 22
            height: 22
            radius: 11
            color: "#30d158"
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: payload.text || "Headphones connected"
            color: Theme.foreground
            font.pixelSize: Theme.textSubtitle
            font.family: Theme.fontFamily
        }
    }
}

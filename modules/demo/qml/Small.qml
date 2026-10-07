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
        spacing: Theme.spaceSmall

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: "headphones"
            color: Theme.success
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: payload.text || "Headphones connected"
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }
}

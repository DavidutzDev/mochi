import QtQuick
import qs.island

// A short line about the connection, like "Connected to Home".
Item {
    id: root

    property var payload: ({})

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 10

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: root.payload.icon ?? "wifi"
            size: 16
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.payload.text ?? ""
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }
    }
}

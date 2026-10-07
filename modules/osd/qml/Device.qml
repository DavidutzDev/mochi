import QtQuick
import qs.island

// The default output changed.
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
            name: payload.kind === "headset" ? "headset" : payload.kind === "display" ? "display" : "speakers"
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, 280)
            text: payload.description ?? ""
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            elide: Text.ElideRight
        }
    }
}

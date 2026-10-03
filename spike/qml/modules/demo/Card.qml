import QtQuick
import qs.island

// A notification-like card. Its height comes from the text, so a longer body
// makes a taller island.
Item {
    property var payload: ({})

    implicitWidth: 380
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: 8

        Row {
            spacing: 8

            Rectangle {
                width: 20
                height: 20
                radius: 5
                color: "#0a84ff"
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Messages"
                color: Theme.muted
                font.pixelSize: 12
            }
        }

        Text {
            text: "Mochi"
            color: Theme.foreground
            font.pixelSize: 15
            font.weight: Font.DemiBold
        }

        Text {
            width: parent.width
            text: payload.text || "The island grows to fit this text. Send a longer body with `mochi-spike ipc demo show Card <text>` and it gets taller."
            color: Theme.foreground
            font.pixelSize: 13
            wrapMode: Text.Wrap
        }
    }
}

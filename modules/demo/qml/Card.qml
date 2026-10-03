import QtQuick
import qs.island

// A notification-like card. Its height comes from the text. Click it for the
// expanded view.
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
                text: "Messages · click to expand"
                color: Theme.muted
                font.pixelSize: 12
            }
        }

        Text {
            width: parent.width
            text: payload.text || "The island grows to fit this text. A longer text makes a taller card."
            color: Theme.foreground
            font.pixelSize: 13
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
    }
}

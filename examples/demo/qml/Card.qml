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
        spacing: Theme.spaceSmall

        Row {
            spacing: Theme.spaceSmall

            Rectangle {
                width: 20
                height: 20
                radius: Theme.radiusControl
                color: Theme.accent

                Symbol {
                    anchors.centerIn: parent
                    name: "chat"
                    size: 14
                    color: Theme.onAccent
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Messages · click to expand"
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        Text {
            width: parent.width
            text: payload.text || "The island grows to fit this text. A longer text makes a taller card."
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
    }
}

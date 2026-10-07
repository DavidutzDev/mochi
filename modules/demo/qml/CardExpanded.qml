import QtQuick
import qs.island

// The card after a click: wider, the full text, and buttons. Its timeout is
// paused while it is open.
Item {
    property var payload: ({})

    implicitWidth: 460
    implicitHeight: column.implicitHeight + Theme.padding * 2

    EdgeLight {
        radius: Theme.radiusSurface
    }

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: Theme.spaceMedium

        Row {
            spacing: Theme.spaceSmall

            Rectangle {
                width: 36
                height: 36
                radius: Theme.radiusControl
                color: Theme.accent

                Symbol {
                    anchors.centerIn: parent
                    name: "chat"
                    color: Theme.onAccent
                }
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    text: "Mochi"
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    text: "Messages · click to collapse, right click to close"
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }
            }
        }

        Text {
            width: parent.width
            text: payload.text || "The island grows to fit this text. A longer text makes a taller card."
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            wrapMode: Text.Wrap
        }

        Row {
            spacing: Theme.spaceSmall

            Repeater {
                model: ["Reply", "Mark as read"]

                Button {
                    required property string modelData

                    text: modelData
                }
            }
        }
    }
}

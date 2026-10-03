import QtQuick
import qs.island

// The card after a click: wider, the full text, and buttons. Its timeout is
// paused while it is open.
Item {
    property var payload: ({})

    implicitWidth: 460
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: 12

        Row {
            spacing: 10

            Rectangle {
                width: 36
                height: 36
                radius: 9
                color: "#0a84ff"
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    text: "Mochi"
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Font.DemiBold
                }

                Text {
                    text: "Messages · click to collapse, right click to close"
                    color: Theme.muted
                    font.pixelSize: Theme.textLabel
                    font.family: Theme.fontFamily
                }
            }
        }

        Text {
            width: parent.width
            text: payload.text || "The island grows to fit this text. A longer text makes a taller card."
            color: Theme.foreground
            font.pixelSize: Theme.textSubtitle
            font.family: Theme.fontFamily
            wrapMode: Text.Wrap
        }

        Row {
            spacing: 8

            Repeater {
                model: ["Reply", "Mark as read"]

                Rectangle {
                    required property string modelData

                    width: label.implicitWidth + 24
                    height: 30
                    radius: 15
                    color: Theme.raised

                    Text {
                        id: label

                        anchors.centerIn: parent
                        text: parent.modelData
                        color: Theme.foreground
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }
                }
            }
        }
    }
}

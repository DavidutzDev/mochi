import QtQuick
import qs.island

// A wide, short layout, like a media player.
Item {
    property var payload: ({})

    implicitWidth: 400
    implicitHeight: 64

    Rectangle {
        id: cover

        anchors.left: parent.left
        anchors.leftMargin: Theme.padding
        anchors.verticalCenter: parent.verticalCenter
        width: 40
        height: 40
        radius: Theme.radiusControl
        gradient: Gradient {
            GradientStop {
                position: 0
                color: Theme.accent
            }
            GradientStop {
                position: 1
                color: Theme.highlight
            }
        }
    }

    Column {
        anchors.left: cover.right
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: parent.right
        anchors.rightMargin: Theme.padding
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceSmall

        Text {
            width: parent.width
            text: payload.text || "Some song - Some artist"
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            elide: Text.ElideRight
        }

        ProgressBar {
            width: parent.width
            value: 0.4
        }
    }
}

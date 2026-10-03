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
        radius: 8
        gradient: Gradient {
            GradientStop { position: 0; color: "#bf5af2" }
            GradientStop { position: 1; color: "#0a84ff" }
        }
    }

    Column {
        anchors.left: cover.right
        anchors.leftMargin: 12
        anchors.right: parent.right
        anchors.rightMargin: Theme.padding
        anchors.verticalCenter: parent.verticalCenter
        spacing: 6

        Text {
            width: parent.width
            text: payload.text || "Some song - Some artist"
            color: Theme.foreground
            font.pixelSize: 14
            elide: Text.ElideRight
        }

        Rectangle {
            width: parent.width
            height: 4
            radius: 2
            color: Theme.surface

            Rectangle {
                width: parent.width * 0.4
                height: parent.height
                radius: 2
                color: Theme.foreground
            }
        }
    }
}

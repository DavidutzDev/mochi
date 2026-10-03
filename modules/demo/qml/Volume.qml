import QtQuick
import qs.island

// A volume OSD. A new level replaces the payload in place, so the bar moves
// instead of the island morphing.
Item {
    property var payload: ({})
    readonly property real level: (payload.level ?? 0) / 100

    implicitWidth: 260
    implicitHeight: 40

    Row {
        anchors.centerIn: parent
        spacing: 12

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Volume"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 150
            height: 6
            radius: 3
            color: Theme.raised

            Rectangle {
                width: parent.width * level
                height: parent.height
                radius: 3
                color: Theme.accent

                Behavior on width {
                    NumberAnimation {
                        duration: 150
                        easing.type: Easing.OutCubic
                    }
                }
            }
        }
    }
}

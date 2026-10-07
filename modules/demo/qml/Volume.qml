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
        spacing: Theme.spaceMedium

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Volume"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        ProgressBar {
            anchors.verticalCenter: parent.verticalCenter
            width: 150
            height: 6
            value: level
            fill: Theme.accent
        }

        RollingText {
            anchors.verticalCenter: parent.verticalCenter
            text: `${Math.round(level * 100)}%`
            pixelSize: Theme.textBody
            weight: Theme.weightLabel
        }
    }
}

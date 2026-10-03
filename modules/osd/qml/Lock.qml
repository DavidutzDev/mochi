import QtQuick
import qs.island

// Caps Lock or Num Lock was toggled.
Item {
    property var payload: ({})
    readonly property bool caps: payload.key === "caps"
    readonly property bool on: payload.on ?? false

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 10

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: caps ? "caps-lock" : "num-lock"
            color: on ? Theme.accent : Theme.muted
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: caps ? "Caps Lock" : "Num Lock"
            color: Theme.foreground
            font.pixelSize: 14
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: on ? "on" : "off"
            color: on ? Theme.accent : Theme.muted
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }
    }
}

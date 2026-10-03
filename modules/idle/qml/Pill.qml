import QtQuick
import Quickshell
import qs.island

Item {
    id: root

    property var payload: ({})

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: Theme.idleHeight

    readonly property string format: payload.format ?? "HH:mm"

    SystemClock {
        id: clock
        // Tick every second only when the format shows seconds.
        precision: root.format.includes("s") ? SystemClock.Seconds : SystemClock.Minutes
    }

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 8

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 7
            height: 7
            radius: 3.5
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Qt.formatTime(clock.date, root.format)
            color: Theme.foreground
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }
    }
}

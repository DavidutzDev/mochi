import QtQuick
import Quickshell
import qs.island

// The resting island: a dot and the time, whose digits roll as it changes.
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
        spacing: Theme.spaceSmall

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 7
            height: 7
            radius: height / 2
            color: Theme.accent
        }

        RollingText {
            anchors.verticalCenter: parent.verticalCenter
            text: Qt.formatTime(clock.date, root.format)
            pixelSize: Theme.textBody
            family: Theme.displayFamily
            weight: Theme.weightTitle
        }
    }
}

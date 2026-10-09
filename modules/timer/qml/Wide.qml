import QtQuick
import qs.island

// The timer's bubble with text, for `wide = true` in `[bubbles.timer]`: a
// small ring and the time left to the second.
Item {
    id: root

    property var payload: ({})
    readonly property color tint: countdown.paused ? Theme.muted : countdown.resting ? Theme.success : Theme.accent
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: `${countdown.what}${countdown.paused ? ", paused" : ""}\nClick to ${countdown.paused ? "resume" : "pause"}`

    implicitWidth: row.implicitWidth + Theme.spaceMedium * 2
    implicitHeight: 26

    Countdown {
        id: countdown

        payload: root.payload
    }

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceSmall

        Ring {
            anchors.verticalCenter: parent.verticalCenter
            width: 14
            height: 14
            progress: countdown.progress
            color: root.tint
        }

        RollingText {
            anchors.verticalCenter: parent.verticalCenter
            text: countdown.text
            color: countdown.paused ? Theme.muted : Theme.foreground
            pixelSize: Theme.textCaption
            weight: Theme.weightTitle
        }
    }
}

import QtQuick
import qs.island

// The bubble while the timer runs: a ring that empties as the phase goes,
// with the minutes left inside it, in the accent for focus and in green
// for a break. Paused, it turns grey with a pause sign. A click pauses or
// resumes it.
Item {
    id: root

    property var payload: ({})
    readonly property color tint: countdown.paused ? Theme.muted : countdown.resting ? Theme.success : Theme.accent
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: `${countdown.what} · ${countdown.text} left${countdown.paused ? ", paused" : ""}\nClick to ${countdown.paused ? "resume" : "pause"}`

    implicitWidth: 26
    implicitHeight: 26

    Countdown {
        id: countdown

        payload: root.payload
    }

    Ring {
        anchors.fill: parent
        progress: countdown.progress
        color: root.tint
    }

    Symbol {
        anchors.centerIn: parent
        visible: countdown.paused
        name: "pause"
        size: 12
        color: root.tint
    }

    Text {
        anchors.centerIn: parent
        visible: !countdown.paused
        // Hours past 99 minutes, so it fits.
        text: countdown.minutes > 99 ? `${Math.ceil(countdown.minutes / 60)}h` : countdown.minutes
        color: Theme.foreground
        font.pixelSize: Theme.textCaption - 1
        font.family: Theme.fontFamily
        font.weight: Theme.weightTitle
        font.features: {
            "tnum": 1
        }
    }
}

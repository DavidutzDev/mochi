import QtQuick
import qs.island

// The bubble while a timer runs: a ring that empties as it goes, with the
// minutes left inside it, in the accent for focus, in green for a break,
// and in the text color for a custom timer, whose last minute counts in
// seconds. Paused, it turns grey with a pause sign. A click pauses or
// resumes it.
Item {
    id: root

    property var payload: ({})
    readonly property int count: payload?.count ?? 1
    readonly property color tint: countdown.paused ? Theme.muted : countdown.custom ? Theme.foreground : countdown.resting ? Theme.success : Theme.accent
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: {
        const more = count > 1 ? `\nand ${count - 1} more, in the clock panel` : "";
        return `${countdown.what} · ${countdown.text} left${countdown.paused ? ", paused" : ""}${more}\nClick to ${countdown.paused ? "resume" : "pause"}`;
    }

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
        text: {
            if (countdown.custom && countdown.remaining < 60000)
                return `${Math.ceil(countdown.remaining / 1000)}s`;
            return countdown.minutes > 99 ? `${Math.ceil(countdown.minutes / 60)}h` : countdown.minutes;
        }
        color: Theme.foreground
        font.pixelSize: Theme.textCaption - 1
        font.family: Theme.fontFamily
        font.weight: Theme.weightTitle
        font.features: {
            "tnum": 1
        }
    }
}

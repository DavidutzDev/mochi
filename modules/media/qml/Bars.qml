import QtQuick
import Quickshell
import qs.island

// Three bars that bounce while playing and settle low when paused. They
// move 30 times a second, and only while they can be seen: not in a hidden
// panel, nor under a fullscreen game, where they'd redraw the island for
// every frame of the monitor.
Row {
    id: root

    property bool playing: false

    spacing: 2

    Ticker {
        id: ticker
        running: root.playing && root.visible && Daemon.shows(root.QsWindow.window) && !Theme.reducedMotion
    }

    Repeater {
        model: 3

        Rectangle {
            id: bar

            required property int index
            // Each bar has its own rhythm, so they never line up: up and
            // down again, from its low to its high, easing at both ends.
            readonly property real low: 4 + index * 2
            readonly property real high: 14 - index * 3
            readonly property real period: Math.max(1, Theme.duration(560 + index * 160))
            readonly property real swing: 0.5 - 0.5 * Math.cos(ticker.now % period / period * 2 * Math.PI)

            anchors.bottom: parent.bottom
            width: 3
            height: root.playing ? low + (high - low) * swing : 4
            radius: 1.5
            color: root.playing ? Theme.accent : Theme.muted
        }
    }

    // The tallest bar, so the row keeps its height while the bars move.
    Item {
        width: 0
        height: 14
    }
}

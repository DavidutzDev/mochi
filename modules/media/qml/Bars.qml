import QtQuick
import qs.island

// Three bars that bounce while playing and settle low when paused.
Row {
    id: root

    property bool playing: false

    spacing: 2

    Repeater {
        model: 3

        Rectangle {
            id: bar

            required property int index

            anchors.bottom: parent.bottom
            width: 3
            height: 4
            radius: 1.5
            color: root.playing ? Theme.accent : Theme.muted

            SequentialAnimation on height {
                running: root.playing
                loops: Animation.Infinite
                // Each bar has its own rhythm, so they never line up.
                NumberAnimation {
                    to: 14 - bar.index * 3
                    duration: 260 + bar.index * 90
                    easing.type: Easing.InOutSine
                }
                NumberAnimation {
                    to: 4 + bar.index * 2
                    duration: 300 + bar.index * 70
                    easing.type: Easing.InOutSine
                }
                onRunningChanged: {
                    if (!running)
                        bar.height = 4;
                }
            }
        }
    }

    // The tallest bar, so the row keeps its height while the bars move.
    Item {
        width: 0
        height: 14
    }
}

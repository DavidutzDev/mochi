import QtQuick
import qs.island

// The bubble while recording: a red dot that breathes. Clicking it opens
// its controls. With `wide = true` in `[bubbles.capture]`, the wide view
// adds the time.
Item {
    property var payload: ({})
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: payload.output ? `Recording ${payload.output} · click for the controls` : "Recording the screen · click for the controls"

    implicitWidth: 26
    implicitHeight: 26

    Rectangle {
        anchors.centerIn: parent
        width: 12
        height: 12
        radius: height / 2
        color: Theme.danger

        SequentialAnimation on opacity {
            loops: Animation.Infinite

            NumberAnimation {
                to: 0.35
                duration: Theme.duration(900)
                easing.type: Easing.InOutSine
            }

            NumberAnimation {
                to: 1
                duration: Theme.duration(900)
                easing.type: Easing.InOutSine
            }
        }
    }
}

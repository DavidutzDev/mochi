import QtQuick
import qs.island

// The bubble while recording: a red dot that breathes. Clicking it stops
// the recording. With `wide = true` in `[bubbles.capture]`, the wide view
// adds the time.
Item {
    property var payload: ({})

    implicitWidth: 26
    implicitHeight: 26

    Rectangle {
        anchors.centerIn: parent
        width: 12
        height: 12
        radius: 6
        color: Theme.danger

        SequentialAnimation on opacity {
            loops: Animation.Infinite

            NumberAnimation {
                to: 0.35
                duration: 900
                easing.type: Easing.InOutSine
            }

            NumberAnimation {
                to: 1
                duration: 900
                easing.type: Easing.InOutSine
            }
        }
    }
}

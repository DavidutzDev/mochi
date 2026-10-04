import QtQuick
import qs.island

// The bubble while something shares the screen: a screen in the accent
// color, breathing, so it's hard to forget.
Item {
    property var payload: ({})

    implicitWidth: 26
    implicitHeight: 26

    Symbol {
        anchors.centerIn: parent
        name: "display"
        size: 16
        color: Theme.accent

        SequentialAnimation on opacity {
            loops: Animation.Infinite

            NumberAnimation {
                to: 0.45
                duration: 1100
                easing.type: Easing.InOutSine
            }

            NumberAnimation {
                to: 1
                duration: 1100
                easing.type: Easing.InOutSine
            }
        }
    }
}

import QtQuick
import qs.island

// Shown while the history is paused: nothing copied is kept. A click
// resumes it.
Item {
    id: root

    property var payload: ({})
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: "Clipboard history paused · click to resume"

    implicitWidth: 26
    implicitHeight: 26

    Symbol {
        anchors.centerIn: parent
        name: "clipboard"
        size: 16
        color: Theme.muted
    }

    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        width: 12
        height: 12
        radius: height / 2
        color: Theme.accent

        Symbol {
            anchors.centerIn: parent
            name: "pause"
            size: 8
            color: Theme.onAccent
        }
    }
}

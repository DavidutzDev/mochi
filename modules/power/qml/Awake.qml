import QtQuick
import qs.island

// The bubble while keep awake is on: a cup, so it isn't left on by
// mistake. A click turns it off.
Item {
    property var payload: ({})
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: "Keep awake is on · click to turn it off"

    implicitWidth: 26
    implicitHeight: 26

    Symbol {
        anchors.centerIn: parent
        name: "coffee"
        size: 16
        color: Theme.accent
        filled: true
    }
}

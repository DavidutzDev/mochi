import QtQuick
import qs.island

// The bubble while keep awake is on: a cup, so it isn't left on by
// mistake. A click turns it off.
Item {
    property var payload: ({})

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

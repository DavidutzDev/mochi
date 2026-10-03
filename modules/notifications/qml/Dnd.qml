import QtQuick
import qs.island

// The bubble while do not disturb is on.
Item {
    property var payload: ({})

    implicitWidth: 26
    implicitHeight: 26

    Symbol {
        anchors.centerIn: parent
        name: "moon"
        size: 16
        color: Theme.muted
    }
}

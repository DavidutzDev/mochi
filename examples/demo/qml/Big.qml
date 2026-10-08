import QtQuick
import qs.island

// A 520x520 view, to check that the island handles very large content.
Item {
    property var payload: ({})

    implicitWidth: 520
    implicitHeight: 520

    Grid {
        anchors.centerIn: parent
        columns: 3
        spacing: Theme.spaceMedium

        Repeater {
            model: 9

            Rectangle {
                required property int index

                width: 152
                height: 152
                radius: Theme.radiusSurface
                color: Qt.hsla(index / 9, 0.6, 0.5, 1)
            }
        }
    }
}

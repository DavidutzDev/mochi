import QtQuick
import qs.island

// While files are dragged over the island: where to let go.
Item {
    property var payload: ({})

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceSmall

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: "place_item"
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Drop to see what to do"
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightLabel
        }
    }
}

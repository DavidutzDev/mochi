import QtQuick
import qs.island

// The island while picking: what to do.
Item {
    property var payload: ({})

    implicitWidth: row.implicitWidth + 28
    implicitHeight: 34

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceSmall

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: "palette"
            size: 16
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Click to pick a color · Esc cancels"
            color: Theme.foreground
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }
}

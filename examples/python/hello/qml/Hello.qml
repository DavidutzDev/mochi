import QtQuick
import qs.island

// The text `say` shows.
Item {
    id: root

    property var payload: ({})

    implicitWidth: label.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Text {
        id: label

        anchors.centerIn: parent
        text: root.payload.text ?? ""
        color: Theme.foreground
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
        font.weight: Font.DemiBold
    }
}

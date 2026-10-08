import QtQuick
import qs.island

// How an action on dropped files went, when its panel was closed before
// it finished.
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
            name: payload.icon ?? "check_circle"
            color: payload.failed ? Theme.danger : Theme.success
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, 420)
            text: payload.text ?? ""
            elide: Text.ElideRight
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightLabel
        }
    }
}

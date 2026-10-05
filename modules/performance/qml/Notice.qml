import QtQuick
import qs.island

// A short line about a reading that stays high, like "CPU at 92% · firefox".
Item {
    id: root

    property var payload: ({})
    readonly property color tint: payload.critical ? Theme.danger : Theme.foreground

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 10

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: root.payload.icon ?? "chip"
            size: 16
            color: root.payload.critical ? Theme.danger : Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.payload.text ?? ""
            color: root.tint
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }
    }
}

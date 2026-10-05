import QtQuick
import qs.island

// A short line about the battery, like "Battery at 50% · 2 h left".
Item {
    id: root

    property var payload: ({})
    readonly property color tint: payload.critical ? Theme.danger : payload.charging ? Theme.accent : Theme.foreground

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 10

        Gauge {
            anchors.verticalCenter: parent.verticalCenter
            level: root.payload.level ?? 0
            charging: root.payload.charging ?? false
            color: root.tint
            size: 15
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.payload.text ?? ""
            color: root.payload.critical ? Theme.danger : Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }
    }
}

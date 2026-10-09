import QtQuick
import qs.island

// A short line about the battery, like "Battery at 50% · 2 h left", or
// about a peripheral's, with its symbol, like "MX Master 3 battery low · 12%".
Item {
    id: root

    property var payload: ({})
    readonly property color tint: payload.critical ? Theme.danger : payload.charging ? Theme.accent : Theme.foreground

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceSmall

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            visible: (root.payload.icon ?? "") !== ""
            name: root.payload.icon ?? ""
            size: 18
            color: root.tint
        }

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
            font.weight: Theme.weightTitle
        }
    }
}

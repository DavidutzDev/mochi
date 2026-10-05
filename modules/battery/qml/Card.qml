import QtQuick
import qs.island

// The hub's home card: the battery's level, and how long until empty or
// full. Without a battery, it says so.
Item {
    id: root

    property var payload: null
    readonly property bool present: payload?.present ?? false
    readonly property color tint: payload?.critical ? Theme.danger : payload?.low ? Theme.accent : Theme.foreground

    implicitHeight: 64

    Text {
        anchors.centerIn: parent
        visible: !root.present
        text: "No battery"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Row {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.present
        spacing: 14

        Gauge {
            anchors.verticalCenter: parent.verticalCenter
            level: root.payload?.level ?? 0
            charging: root.payload?.charging ?? false
            color: root.tint
            size: 26
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2

            Text {
                text: `${root.payload?.level ?? 0}%`
                color: root.tint
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
            }

            Text {
                text: root.payload?.state ?? ""
                color: Theme.muted
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }
        }
    }
}

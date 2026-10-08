import QtQuick
import qs.island

// A recording's controls, from a click on its bubble: how long it has
// recorded, the screens a whole-screen recording can go on to, the one it
// records highlighted, and Stop.
Item {
    id: root

    property var payload: ({})
    readonly property var screens: payload.screens ?? []

    implicitWidth: row.implicitWidth + 12
    implicitHeight: row.implicitHeight + 8

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceTiny

        Item {
            width: Theme.spaceTiny
            height: 1
        }

        RecordingWide {
            anchors.verticalCenter: parent.verticalCenter
            payload: root.payload
        }

        Item {
            width: Theme.spaceSmall
            height: 1
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.screens.length > 1
            width: 1
            height: 18
            color: Theme.raised
        }

        Repeater {
            model: root.screens.length > 1 ? root.screens : []

            Button {
                required property string modelData

                anchors.verticalCenter: parent.verticalCenter
                icon: "display"
                text: modelData
                tone: root.payload.output === modelData ? "accent" : "ghost"
                onClicked: {
                    if (root.payload.output !== modelData)
                        Daemon.command("capture", "switch", [modelData]);
                }
            }
        }

        Button {
            anchors.verticalCenter: parent.verticalCenter
            icon: "stop"
            text: "Stop"
            tone: "danger"
            onClicked: Daemon.command("capture", "stop", [])
        }
    }
}

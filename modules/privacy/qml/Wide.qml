import QtQuick
import qs.island

// The privacy bubble with text, for `wide = true`: the microphone and the
// camera, each with the apps using it.
Item {
    id: root

    property var payload: ({})
    readonly property var microphone: payload.microphone ?? []
    readonly property var camera: payload.camera ?? []

    implicitWidth: row.implicitWidth + Theme.spaceMedium * 2
    implicitHeight: 26

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceMedium

        Repeater {
            model: [
                {
                    "apps": root.microphone,
                    "icon": root.payload.muted ? "mic_off" : "mic",
                    "color": Theme.accent
                },
                {
                    "apps": root.camera,
                    "icon": "videocam",
                    "color": Theme.success
                }
            ].filter(use => use.apps.length > 0)

            Row {
                required property var modelData

                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceTiny

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: modelData.icon
                    size: 14
                    color: modelData.color
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.apps.join(", ")
                    color: Theme.foreground
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                }
            }
        }
    }
}

import QtQuick
import qs.island

// The bubble while an app uses the microphone or the camera: an orange dot
// for the microphone, a green one for the camera. A muted microphone's dot
// is a ring, since whoever records hears nothing.
Item {
    id: root

    property var payload: ({})
    readonly property bool microphone: (payload.microphone ?? []).length > 0
    readonly property bool camera: (payload.camera ?? []).length > 0

    implicitWidth: dots.implicitWidth + Theme.spaceSmall * 2
    implicitHeight: 26

    Row {
        id: dots

        anchors.centerIn: parent
        spacing: Theme.spaceTiny

        Rectangle {
            visible: root.microphone
            width: 8
            height: 8
            radius: width / 2
            color: root.payload.muted ? "transparent" : Theme.accent
            border.width: root.payload.muted ? 2 : 0
            border.color: Theme.accent
        }

        Rectangle {
            visible: root.camera
            width: 8
            height: 8
            radius: width / 2
            color: Theme.success
        }
    }
}

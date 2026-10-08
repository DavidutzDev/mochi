import QtQuick
import qs.island

// The bubble while an app uses the microphone or the camera: a microphone
// in orange, a camera in green, or both. A muted microphone shows crossed
// out, since whoever records hears nothing.
Item {
    id: root

    property var payload: ({})
    readonly property bool microphone: (payload.microphone ?? []).length > 0
    readonly property bool camera: (payload.camera ?? []).length > 0
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: {
        const lines = [];
        if (microphone)
            lines.push(`Microphone${payload.muted ? " (muted)" : ""}: ${payload.microphone.join(", ")}`);
        if (camera)
            lines.push(`Camera: ${payload.camera.join(", ")}`);
        return lines.join("\n");
    }

    implicitWidth: icons.implicitWidth + Theme.spaceSmall * 2
    implicitHeight: 26

    Row {
        id: icons

        anchors.centerIn: parent
        spacing: Theme.spaceTiny

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.microphone
            name: root.payload.muted ? "mic_off" : "mic"
            size: 16
            filled: true
            color: root.payload.muted ? Theme.muted : Theme.accent
        }

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.camera
            name: "videocam"
            size: 16
            filled: true
            color: Theme.success
        }
    }
}

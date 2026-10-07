import QtQuick
import qs.island

// The small bubble: the name's first letter in a circle. Small views fit in
// about 26 pixels; the round bubble around them adds the background.
Rectangle {
    property var payload: ({})

    implicitWidth: 22
    implicitHeight: 22
    radius: height / 2
    color: Theme.accent

    Text {
        anchors.centerIn: parent
        text: (payload.text ?? "?").charAt(0).toUpperCase()
        color: Theme.onAccent
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
        font.weight: Theme.weightTitle
    }
}

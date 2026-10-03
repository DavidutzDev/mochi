import QtQuick
import qs.island

// The wide bubble: a dot and the name. Bubble views size to their content;
// the pill around them adds the padding and the background.
Row {
    property var payload: ({})

    spacing: 6

    Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: 8
        height: 8
        radius: 4
        color: Theme.accent
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: payload.text ?? ""
        color: Theme.foreground
        font.pixelSize: 13
    }
}

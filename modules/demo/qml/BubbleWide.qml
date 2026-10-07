import QtQuick
import qs.island

// The wide bubble: a dot and the name. Bubble views size to their content;
// the pill around them adds the padding and the background.
Row {
    property var payload: ({})

    spacing: Theme.spaceSmall

    Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: 8
        height: 8
        radius: height / 2
        color: Theme.accent
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: payload.text ?? ""
        color: Theme.foreground
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }
}

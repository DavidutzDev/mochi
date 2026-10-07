import QtQuick
import qs.island

// The wide bubble, when the user asks for it: the cover, the title and the
// bars. Paused, the title dims and the bars settle.
Row {
    id: root

    property var payload: ({})
    readonly property bool playing: payload.status === "playing"

    spacing: Theme.spaceSmall

    Art {
        anchors.verticalCenter: parent.verticalCenter
        source: root.payload.art ?? ""
        size: 20
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(implicitWidth, 160)
        text: root.payload.title ?? ""
        elide: Text.ElideRight
        color: root.playing ? Theme.foreground : Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
        font.weight: Theme.weightTitle

        Behavior on color {
            ColorAnimation {
                duration: Theme.fast
            }
        }
    }

    Bars {
        anchors.verticalCenter: parent.verticalCenter
        playing: root.playing
    }
}

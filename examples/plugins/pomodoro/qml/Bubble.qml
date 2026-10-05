import QtQuick
import qs.island
import "Clock.js" as Clock

// The countdown while a timer runs: a ring and the time left. Muted while
// paused; a click pauses or resumes.
Item {
    id: root

    property var payload: ({})
    readonly property bool resting: payload.phase === "break"
    readonly property color tint: payload.paused ? Theme.muted : resting ? Theme.foreground : Theme.accent

    implicitWidth: row.implicitWidth + 12
    implicitHeight: 26

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 5

        Ring {
            anchors.verticalCenter: parent.verticalCenter
            width: 15
            height: 15
            line: 2
            color: root.tint
            progress: (root.payload.left ?? 0) / Math.max(1, root.payload.total ?? 1)
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Clock.format(root.payload.left)
            color: root.tint
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
            font.features: { "tnum": 1 }
        }
    }
}

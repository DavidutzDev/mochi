import QtQuick
import qs.island

// The cover, the title and a playing indicator. Paused, the title dims and
// the bars stop.
Item {
    id: root

    property var payload: ({})
    readonly property bool playing: payload.status === "playing"

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 10

        Art {
            anchors.verticalCenter: parent.verticalCenter
            source: root.payload.art ?? ""
            size: 24
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, 220)
            text: root.payload.title ?? ""
            elide: Text.ElideRight
            color: root.playing ? Theme.foreground : Theme.muted
            font.pixelSize: 14
            font.weight: Font.DemiBold

            Behavior on color {
                ColorAnimation {
                    duration: 200
                }
            }
        }

        Bars {
            anchors.verticalCenter: parent.verticalCenter
            playing: root.playing
        }
    }
}

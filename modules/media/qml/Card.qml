import QtQuick
import qs.island

// Now playing, as a hub card: the cover, the track, progress and controls.
// It reads the media module's state, so it shows whatever the island would.
Item {
    id: root

    property var payload: null
    readonly property bool playing: payload?.status === "playing"

    Text {
        anchors.centerIn: parent
        visible: root.payload === null
        text: "Nothing playing"
        color: Theme.muted
        font.pixelSize: 13
    }

    Position {
        id: clock

        payload: root.payload ?? ({})
    }

    Row {
        anchors.fill: parent
        visible: root.payload !== null
        spacing: 16

        Art {
            id: art

            anchors.verticalCenter: parent.verticalCenter
            source: root.payload?.art ?? ""
            size: Math.min(parent.height, 112)
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - art.width - parent.spacing
            spacing: 4

            Text {
                width: parent.width
                text: root.payload?.title ?? ""
                elide: Text.ElideRight
                color: Theme.foreground
                font.pixelSize: 17
                font.weight: Font.DemiBold
            }

            Text {
                width: parent.width
                text: [root.payload?.artist, root.payload?.player].filter(part => part).join(" · ")
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: 12
            }

            Item {
                width: parent.width
                height: 14
                visible: clock.length > 0

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    height: 4
                    radius: 2
                    color: Qt.rgba(1, 1, 1, 0.12)

                    Rectangle {
                        width: parent.width * clock.progress
                        height: parent.height
                        radius: parent.radius
                        color: Theme.foreground
                    }
                }
            }

            Row {
                spacing: 10

                Button {
                    icon: "previous"
                    action: "previous"
                    size: 18
                    enabled: root.payload?.can_previous ?? false
                }

                Button {
                    icon: root.playing ? "pause" : "play"
                    action: "play-pause"
                    size: 22
                    enabled: root.payload?.can_play_pause ?? false
                }

                Button {
                    icon: "next"
                    action: "next"
                    size: 18
                    enabled: root.payload?.can_next ?? false
                }
            }
        }
    }
}

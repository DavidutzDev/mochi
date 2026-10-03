import QtQuick
import qs.island

// Now playing, as a hub card: the cover, the track, progress and controls.
// It reads the media module's state, so it shows whatever the island would.
Item {
    id: root

    property var payload: null
    readonly property bool playing: payload?.status === "playing"

    implicitHeight: payload === null ? 40 : 96

    Text {
        anchors.centerIn: parent
        visible: root.payload === null
        text: "Nothing playing"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
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
            size: 96
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
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }

            Text {
                width: parent.width
                text: [root.payload?.artist, root.payload?.player].filter(part => part).join(" · ")
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }

            ProgressBar {
                width: parent.width
                visible: clock.length > 0
                value: clock.progress
            }

            Row {
                spacing: 10

                IconButton {
                    icon: "previous"
                    size: 18
                    enabled: root.payload?.can_previous ?? false
                    onClicked: Daemon.command("media", "previous", [])
                }

                IconButton {
                    icon: root.playing ? "pause" : "play"
                    size: 22
                    enabled: root.payload?.can_play_pause ?? false
                    onClicked: Daemon.command("media", "play-pause", [])
                }

                IconButton {
                    icon: "next"
                    size: 18
                    enabled: root.payload?.can_next ?? false
                    onClicked: Daemon.command("media", "next", [])
                }
            }
        }
    }
}

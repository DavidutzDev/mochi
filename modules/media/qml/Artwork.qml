import QtQuick
import qs.island

// The now playing widget's artwork look, a tall card: the cover as wide as
// the widget, the title and the artist under it, the progress, waving
// while the track plays, and the controls, play and pause in the accent
// color. It reads the media module's state, so it shows whatever the
// island would, and steps aside while nothing plays.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool playing: payload?.status === "playing"
    readonly property bool hidden: payload === null

    Position {
        id: clock

        payload: root.payload ?? ({})
    }

    function time(milliseconds: real): string {
        const seconds = Math.floor(milliseconds / 1000);
        const minutes = Math.floor(seconds / 60);
        const rest = String(seconds % 60).padStart(2, "0");
        if (minutes < 60)
            return `${minutes}:${rest}`;
        return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, "0")}:${rest}`;
    }

    Art {
        id: art

        anchors.horizontalCenter: parent.horizontalCenter
        visible: root.payload !== null
        source: root.payload?.art ?? ""
        // Square, in what the lines under it leave.
        size: Math.max(0, Math.min(root.width, root.height - below.implicitHeight - Theme.spaceMedium))
        radius: Theme.radiusField
    }

    Column {
        id: below

        anchors.top: art.bottom
        anchors.topMargin: Theme.spaceMedium
        width: parent.width
        visible: root.payload !== null
        spacing: Theme.spaceTiny

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.payload?.title ?? ""
            color: Theme.foreground
            font.pixelSize: Theme.textTitle
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Text {
            width: parent.width
            visible: text !== ""
            elide: Text.ElideRight
            text: [root.payload?.artist, root.payload?.player].filter(part => part).join(" · ")
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Item {
            width: 1
            height: Theme.spaceTiny
        }

        // Seeks on a click or a drag, where the player can.
        WavyProgress {
            width: parent.width
            visible: clock.length > 0
            playing: root.playing
            enabled: root.payload?.can_seek ?? false
            value: clock.progress
            onReleased: value => Daemon.command("media", "seek", [(value * clock.length / 1000).toFixed(2)])
        }

        Item {
            width: parent.width
            height: elapsed.implicitHeight
            visible: clock.length > 0

            Text {
                id: elapsed

                text: root.time(clock.position)
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.features: {
                    "tnum": 1
                }
            }

            Text {
                anchors.right: parent.right
                text: root.time(clock.length)
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.features: {
                    "tnum": 1
                }
            }
        }

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: Theme.spaceMedium

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                icon: "previous"
                size: 18
                enabled: root.payload?.can_previous ?? false
                onClicked: Daemon.command("media", "previous", [])
            }

            Button {
                anchors.verticalCenter: parent.verticalCenter
                implicitHeight: Theme.rowHeight
                icon: root.playing ? "pause" : "play"
                iconSize: 22
                tone: "accent"
                enabled: root.payload?.can_play_pause ?? false
                onClicked: Daemon.command("media", "play-pause", [])
            }

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                icon: "next"
                size: 18
                enabled: root.payload?.can_next ?? false
                onClicked: Daemon.command("media", "next", [])
            }
        }
    }
}

import QtQuick
import qs.island

// The cover, title, artist and player, a progress line that waves while the
// track plays and seeks on click or drag, the controls, and the player's own volume when it has one.
Item {
    id: root

    property var payload: ({})
    readonly property bool playing: payload.status === "playing"
    readonly property real length: clock.length
    readonly property real position: clock.position

    // Where the pointer seeks to, or -1. It holds until the player reports
    // the new position.
    property real seeking: -1
    readonly property real shownPosition: seeking >= 0 ? seeking : position
    // The width of the times beside the progress bar, and of the volume's
    // icon and percent, so both bars line up.
    readonly property real side: Math.max(total.implicitWidth, volume.visible ? volume.percentWidth : 0)

    onPayloadChanged: {
        if (!bar.dragging)
            seeking = -1;
    }

    implicitWidth: 380
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Position {
        id: clock

        payload: root.payload
    }

    function time(milliseconds: real): string {
        const seconds = Math.floor(milliseconds / 1000);
        const minutes = Math.floor(seconds / 60);
        const rest = String(seconds % 60).padStart(2, "0");
        if (minutes < 60)
            return `${minutes}:${rest}`;
        return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, "0")}:${rest}`;
    }

    EdgeLight {
        radius: Theme.radiusSurface
    }

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: Theme.spaceMedium

        Row {
            width: parent.width
            spacing: Theme.spaceMedium

            Art {
                id: art

                source: root.payload.art ?? ""
                size: 64
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - art.width - parent.spacing
                spacing: 2

                Players {
                    width: Math.min(implicitWidth, parent.width)
                    payload: root.payload
                }

                Text {
                    width: parent.width
                    text: root.payload.title ?? ""
                    elide: Text.ElideRight
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    visible: text !== ""
                    text: root.payload.artist ?? ""
                    elide: Text.ElideRight
                    color: Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }
        }

        Row {
            width: parent.width
            visible: root.length > 0
            spacing: Theme.spaceSmall

            Text {
                id: elapsed

                anchors.verticalCenter: parent.verticalCenter
                width: root.side
                horizontalAlignment: Text.AlignRight
                text: root.time(root.shownPosition)
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.features: {
                    "tnum": 1
                }
            }

            WavyProgress {
                id: bar

                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - elapsed.width - total.width - parent.spacing * 2
                playing: root.playing
                enabled: root.payload.can_seek ?? false
                value: root.length > 0 ? root.shownPosition / root.length : 0
                onMoved: value => root.seeking = value * root.length
                onReleased: value => {
                    root.seeking = value * root.length;
                    Daemon.command("media", "seek", [(root.seeking / 1000).toFixed(2)]);
                }
            }

            Text {
                id: total

                anchors.verticalCenter: parent.verticalCenter
                width: root.side
                text: root.time(root.length)
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
            spacing: Theme.spaceLarge

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                icon: "previous"
                enabled: root.payload.can_previous ?? false
                size: 20
                onClicked: Daemon.command("media", "previous", [])
            }

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                icon: root.playing ? "pause" : "play"
                size: 26
                enabled: root.payload.can_play_pause ?? false
                onClicked: Daemon.command("media", "play-pause", [])
            }

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                icon: "next"
                enabled: root.payload.can_next ?? false
                size: 20
                onClicked: Daemon.command("media", "next", [])
            }
        }

        PlayerVolume {
            id: volume

            width: parent.width
            payload: root.payload
            side: root.side
        }
    }
}

import QtQuick
import qs.island

// Now playing, as a control center card and a desktop widget: the cover as tall
// as the card, the track at the top, and progress, controls and the player's
// own volume at the bottom.
// It fills the size it's given. It reads the media module's state, so it shows
// whatever the island would.
Item {
    id: root

    property var payload: null
    // What the desktop gives a widget; this one has no settings.
    property var settings: ({})
    property string instance: ""
    readonly property bool playing: payload?.status === "playing"
    // Nothing playing: the card makes room for the others.
    readonly property bool hidden: payload === null

    implicitWidth: 360
    implicitHeight: Theme.tileHeight

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

        anchors.verticalCenter: parent.verticalCenter
        visible: root.payload !== null
        source: root.payload?.art ?? ""
        size: Math.min(root.height, root.width * 0.4)
    }

    Item {
        id: details

        anchors.left: art.right
        anchors.leftMargin: Theme.spaceLarge
        anchors.right: parent.right
        anchors.top: art.top
        anchors.bottom: art.bottom
        visible: root.payload !== null

        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top

            Text {
                width: parent.width
                text: root.payload?.title ?? ""
                elide: Text.ElideRight
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                visible: text !== ""
                // With several players, the arrows below name the player.
                text: [root.payload?.artist, players.several ? "" : root.payload?.player].filter(part => part).join(" · ")
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }
        }

        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            spacing: Theme.spaceTiny

            Row {
                width: parent.width
                visible: clock.length > 0
                spacing: Theme.spaceSmall

                Text {
                    id: elapsed

                    anchors.verticalCenter: parent.verticalCenter
                    width: total.implicitWidth
                    horizontalAlignment: Text.AlignRight
                    text: root.time(clock.position)
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.features: {
                        "tnum": 1
                    }
                }

                ProgressBar {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - elapsed.width - total.width - parent.spacing * 2
                    value: clock.progress
                }

                Text {
                    id: total

                    anchors.verticalCenter: parent.verticalCenter
                    text: root.time(clock.length)
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.features: {
                        "tnum": 1
                    }
                }
            }

            Item {
                width: parent.width
                height: controls.height

                Row {
                    id: controls

                    spacing: Theme.spaceSmall

                    IconButton {
                        anchors.verticalCenter: parent.verticalCenter
                        icon: "previous"
                        size: 18
                        enabled: root.payload?.can_previous ?? false
                        onClicked: Daemon.command("media", "previous", [])
                    }

                    IconButton {
                        anchors.verticalCenter: parent.verticalCenter
                        icon: root.playing ? "pause" : "play"
                        size: 22
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

                // The player's own volume, in the room the controls and the
                // arrows leave; none when that's too narrow for a slider.
                PlayerVolume {
                    anchors.left: controls.right
                    anchors.leftMargin: Theme.spaceMedium
                    anchors.right: players.visible ? players.left : parent.right
                    anchors.rightMargin: players.visible ? Theme.spaceMedium : 0
                    anchors.verticalCenter: parent.verticalCenter
                    visible: available && width >= 64
                    payload: root.payload
                    percent: false
                }

                Players {
                    id: players

                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, parent.width - controls.width - Theme.spaceMedium)
                    visible: several
                    payload: root.payload
                }
            }
        }
    }
}

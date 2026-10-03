import QtQuick
import qs.island

// The cover, title, artist and player, a progress bar that seeks on click or
// drag, and the controls.
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

    onPayloadChanged: {
        if (!bar.pressed)
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

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: 12

        Row {
            width: parent.width
            spacing: 14

            Art {
                id: art

                source: root.payload.art ?? ""
                size: 64
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - art.width - parent.spacing
                spacing: 2

                Text {
                    width: parent.width
                    text: root.payload.player ?? ""
                    elide: Text.ElideRight
                    color: Theme.muted
                    font.pixelSize: 11
                }

                Text {
                    width: parent.width
                    text: root.payload.title ?? ""
                    elide: Text.ElideRight
                    color: Theme.foreground
                    font.pixelSize: 15
                    font.weight: Font.DemiBold
                }

                Text {
                    width: parent.width
                    visible: text !== ""
                    text: root.payload.artist ?? ""
                    elide: Text.ElideRight
                    color: Theme.muted
                    font.pixelSize: 13
                }
            }
        }

        Row {
            width: parent.width
            visible: root.length > 0
            spacing: 10

            Text {
                id: elapsed

                anchors.verticalCenter: parent.verticalCenter
                width: total.implicitWidth
                horizontalAlignment: Text.AlignRight
                text: root.time(root.shownPosition)
                color: Theme.muted
                font.pixelSize: 11
                font.features: { "tnum": 1 }
            }

            Item {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - elapsed.width - total.width - parent.spacing * 2
                height: 14

                Rectangle {
                    id: track

                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    height: bar.containsMouse || bar.pressed ? 6 : 4
                    radius: height / 2
                    color: Theme.surface

                    Behavior on height {
                        NumberAnimation {
                            duration: 120
                        }
                    }

                    Rectangle {
                        width: parent.width * Math.min(root.shownPosition / Math.max(root.length, 1), 1)
                        height: parent.height
                        radius: parent.radius
                        color: Theme.foreground
                    }
                }

                MouseArea {
                    id: bar

                    // A taller target than the thin bar.
                    anchors.fill: parent
                    enabled: root.payload.can_seek ?? false
                    hoverEnabled: true
                    cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor

                    function at(x: real): real {
                        return Math.max(0, Math.min(x / width, 1)) * root.length;
                    }

                    onPressed: mouse => root.seeking = at(mouse.x)
                    onPositionChanged: mouse => {
                        if (pressed)
                            root.seeking = at(mouse.x);
                    }
                    onReleased: mouse => {
                        root.seeking = at(mouse.x);
                        Daemon.command("media", "seek", [(root.seeking / 1000).toFixed(2)]);
                    }
                }
            }

            Text {
                id: total

                anchors.verticalCenter: parent.verticalCenter
                text: root.time(root.length)
                color: Theme.muted
                font.pixelSize: 11
                font.features: { "tnum": 1 }
            }
        }

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 18

            Button {
                anchors.verticalCenter: parent.verticalCenter
                icon: "previous"
                action: "previous"
                enabled: root.payload.can_previous ?? false
            }

            Button {
                anchors.verticalCenter: parent.verticalCenter
                icon: root.playing ? "pause" : "play"
                action: "play-pause"
                size: 26
                enabled: root.payload.can_play_pause ?? false
            }

            Button {
                anchors.verticalCenter: parent.verticalCenter
                icon: "next"
                action: "next"
                enabled: root.payload.can_next ?? false
            }
        }
    }
}

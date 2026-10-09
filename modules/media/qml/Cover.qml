import QtQuick
import QtQuick.Effects
import qs.island

// The now playing widget's cover look: only the cover, without a card,
// rounded like one and lifted by the same shadow, with a button in its
// corner to play or pause. It steps aside while nothing plays.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool playing: payload?.status === "playing"
    readonly property bool hidden: payload === null

    RectangularShadow {
        anchors.fill: art
        visible: art.visible && Theme.shadow.a > 0
        radius: art.radius
        blur: 20
        offset.y: 3
        color: Theme.shadow
    }

    Art {
        id: art

        anchors.centerIn: parent
        visible: root.payload !== null
        source: root.payload?.art ?? ""
        size: Math.min(root.width, root.height)
        radius: Theme.radiusSurface
    }

    Button {
        anchors.right: art.right
        anchors.bottom: art.bottom
        anchors.margins: Theme.spaceMedium
        visible: root.payload !== null
        implicitHeight: Math.max(Theme.controlHeight, Math.min(Theme.rowHeight, art.size * 0.22))
        icon: root.playing ? "pause" : "play"
        iconSize: implicitHeight * 0.5
        tone: "accent"
        enabled: root.payload?.can_play_pause ?? false
        onClicked: Daemon.command("media", "play-pause", [])
    }
}

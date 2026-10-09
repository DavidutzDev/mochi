import QtQuick
import qs.island

// The control center's home card: the output's volume as a slider, the icon
// muting it. The heading opens the mixer. Without the audio server, it steps
// aside.
Item {
    id: root

    property var payload: null
    readonly property bool hidden: !(payload?.connected ?? false) || payload?.output == null
    readonly property var output: payload?.output ?? null
    readonly property int maxVolume: payload?.max_volume ?? 100

    implicitHeight: Theme.rowHeight

    // Dragging sends a level at most every 50 ms; letting go sends the last.
    property int pending: -1

    function send(level: int): void {
        Daemon.command("audio", "volume", ["output", `${level}`]);
    }

    Timer {
        id: throttle

        interval: 50
        onTriggered: {
            if (root.pending >= 0)
                root.send(root.pending);
            root.pending = -1;
        }
    }

    SliderRow {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        icon: root.output?.muted ? "volume-muted" : "volume"
        value: Math.min(root.output?.volume ?? 0, root.maxVolume) / root.maxVolume
        maximum: root.maxVolume
        onMoved: value => {
            root.pending = Math.round(value * root.maxVolume);
            if (!throttle.running)
                throttle.start();
        }
        onReleased: value => {
            throttle.stop();
            root.pending = -1;
            root.send(Math.round(value * root.maxVolume));
        }
        onIconClicked: Daemon.command("audio", "mute", ["output"])
    }
}

import QtQuick
import qs.island

// The player's own volume, apart from the app's volume in the mixer: an
// icon, a thin slider and the percent. Players that don't report a volume,
// or take no commands, get none, so it hides. `side` is the width the icon
// and the percent each take, to line the slider up with a progress bar
// above it; `percent` off leaves the number out where room is short.
Item {
    id: root

    property var payload: ({})
    property real side: 0
    property bool percent: true
    readonly property bool available: payload?.can_volume ?? false
    readonly property int volume: payload?.volume ?? 0
    // How wide "100%" is, for a `side` that fits it.
    readonly property real percentWidth: widest.advanceWidth

    visible: available
    implicitWidth: 200
    implicitHeight: 20

    function send(level: int): void {
        Daemon.command("media", "volume", [`${level}`]);
    }

    // Dragging sends a level at most every 50 ms; letting go sends the last.
    property int pending: -1

    Timer {
        id: throttle

        interval: 50
        onTriggered: {
            if (root.pending >= 0)
                root.send(root.pending);
            root.pending = -1;
        }
    }

    Item {
        id: leading

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(root.side, icon.size)
        height: icon.size

        Symbol {
            id: icon

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            name: {
                const shown = slider.shownPercent;
                if (shown === 0)
                    return "volume-muted";
                if (shown < 34)
                    return "volume-1";
                if (shown < 67)
                    return "volume-2";
                return "volume-3";
            }
            size: 16
            color: Theme.muted
        }
    }

    Slider {
        id: slider

        anchors.left: leading.right
        anchors.leftMargin: Theme.spaceSmall
        anchors.right: trailing.visible ? trailing.left : parent.right
        anchors.rightMargin: trailing.visible ? Theme.spaceSmall : 0
        anchors.verticalCenter: parent.verticalCenter
        thickness: 4
        value: root.volume / 100
        // A double click puts it back to full.
        reset: 1

        readonly property int shownPercent: dragging ? Math.round(shown * 100) : root.volume

        onMoved: value => {
            root.pending = Math.round(value * 100);
            if (!throttle.running)
                throttle.start();
        }
        onReleased: value => {
            throttle.stop();
            root.pending = -1;
            root.send(Math.round(value * 100));
        }
    }

    Item {
        id: trailing

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        visible: root.percent
        width: Math.max(root.side, widest.advanceWidth)
        height: number.height

        RollingText {
            id: number

            anchors.left: parent.left
            text: `${slider.shownPercent}%`
            color: Theme.muted
            pixelSize: Theme.textCaption
        }

        TextMetrics {
            id: widest

            font: number.font
            text: "100%"
        }
    }
}

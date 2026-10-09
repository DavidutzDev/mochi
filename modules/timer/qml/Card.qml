import QtQuick
import qs.island

// The control center's card: the time left in a ring, with buttons to pause
// and stop, or the length of a focus session and a button to start one.
Item {
    id: root

    property var payload: null
    readonly property int sessions: payload?.sessions ?? 0

    implicitHeight: Theme.rowHeight

    Countdown {
        id: countdown

        payload: root.payload
    }

    Ring {
        id: ring

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: 34
        height: 34
        line: 3
        progress: countdown.running ? countdown.progress : 0
        color: countdown.paused ? Theme.muted : countdown.resting ? Theme.success : Theme.accent
    }

    Column {
        anchors.left: ring.right
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: buttons.left
        anchors.rightMargin: Theme.spaceSmall
        anchors.verticalCenter: parent.verticalCenter

        RollingText {
            text: countdown.running ? countdown.text : `${root.payload?.focus_minutes ?? 25}:00`
            color: countdown.running ? Theme.foreground : Theme.muted
            pixelSize: Theme.textTitle
            weight: Theme.weightTitle
        }

        Text {
            width: parent.width
            text: {
                if (countdown.running)
                    return countdown.paused ? `${countdown.what}, paused` : countdown.what;
                if (root.sessions === 0)
                    return "Ready";
                return root.sessions === 1 ? "1 session done" : `${root.sessions} sessions done`;
            }
            elide: Text.ElideRight
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }

    Row {
        id: buttons

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        Button {
            visible: !countdown.running
            text: "Start"
            icon: "play"
            tone: "accent"
            onClicked: Daemon.command("timer", "start", [])
        }

        IconButton {
            visible: countdown.running
            icon: countdown.paused ? "play" : "pause"
            size: 16
            onClicked: Daemon.command("timer", countdown.paused ? "resume" : "pause", [])
        }

        IconButton {
            visible: countdown.running
            icon: "stop"
            size: 16
            onClicked: Daemon.command("timer", "stop", [])
        }
    }
}

import QtQuick
import qs.island

// The focus timer as a desktop widget: the time left big inside a ring that
// empties as the phase runs, what's running under it, and buttons to pause
// or resume and to stop. The ring waves while it counts down and lies flat
// while paused. With nothing running, the length of a focus session in an
// empty ring and a button to start one.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""

    readonly property int sessions: payload?.sessions ?? 0

    Countdown {
        id: countdown

        payload: root.payload
    }

    readonly property real size: Math.max(0, Math.min(width, height - buttons.height - Theme.spaceMedium))

    WavyRing {
        id: ring

        anchors.horizontalCenter: parent.horizontalCenter
        width: root.size
        height: root.size
        size: root.size
        thickness: Math.max(4, root.size * 0.06)
        value: countdown.running ? countdown.progress : 0
        wavy: countdown.running && !countdown.paused
        color: countdown.paused ? Theme.muted : countdown.resting ? Theme.success : Theme.accent

        Column {
            anchors.centerIn: parent

            RollingText {
                anchors.horizontalCenter: parent.horizontalCenter
                text: countdown.running ? countdown.text : `${root.payload?.focus_minutes ?? 25}:00`
                color: countdown.running ? Theme.foreground : Theme.muted
                family: Theme.displayFamily
                weight: Theme.weightTitle
                pixelSize: Math.max(Theme.textTitle, Math.min(Theme.textDisplay, root.size * 0.2))
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: {
                    if (countdown.running)
                        return countdown.paused ? `${countdown.what}, paused` : countdown.what;
                    if (root.sessions === 0)
                        return "Ready to focus";
                    return root.sessions === 1 ? "1 session done" : `${root.sessions} sessions done`;
                }
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }
        }
    }

    Row {
        id: buttons

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: ring.bottom
        anchors.topMargin: Theme.spaceMedium
        spacing: Theme.spaceSmall

        Button {
            visible: !countdown.running
            text: "Start focus"
            icon: "play"
            tone: "accent"
            onClicked: Daemon.command("timer", "start", [])
        }

        Button {
            visible: countdown.running
            icon: countdown.paused ? "play" : "pause"
            tone: countdown.paused ? "accent" : "neutral"
            onClicked: Daemon.command("timer", countdown.paused ? "resume" : "pause", [])
        }

        Button {
            visible: countdown.running
            icon: "stop"
            onClicked: Daemon.command("timer", "stop", [])
        }
    }
}

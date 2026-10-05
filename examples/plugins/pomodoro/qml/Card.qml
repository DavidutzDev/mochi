import QtQuick
import qs.island
import "Clock.js" as Clock

// The hub's card: the time left and buttons for the timer, or a button to
// start one.
Item {
    id: root

    property var payload: null
    readonly property string phase: payload?.phase ?? "idle"
    readonly property bool running: phase !== "idle"

    implicitHeight: 64

    Row {
        anchors.verticalCenter: parent.verticalCenter
        spacing: 14

        Ring {
            anchors.verticalCenter: parent.verticalCenter
            width: 34
            height: 34
            line: 3
            color: root.payload?.paused ? Theme.muted : Theme.accent
            progress: root.running ? (root.payload.left ?? 0) / Math.max(1, root.payload.total ?? 1) : 0
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2

            Text {
                text: root.running ? Clock.format(root.payload.left) : `${root.payload?.focus_minutes ?? 25} min`
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
            }

            Text {
                text: {
                    const done = root.payload?.sessions ?? 0;
                    const sessions = done === 1 ? "1 session" : `${done} sessions`;
                    if (!root.running)
                        return `Ready · ${sessions} done`;
                    const what = root.phase === "break" ? "Break" : "Focus";
                    return root.payload.paused ? `${what}, paused` : what;
                }
                color: Theme.muted
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }
        }
    }

    Row {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 6

        Button {
            visible: !root.running
            text: "Start"
            tone: "accent"
            onClicked: Daemon.command("pomodoro", "start", [])
        }

        Button {
            visible: root.running
            icon: root.payload?.paused ? "play" : "pause"
            onClicked: Daemon.command("pomodoro", "pause", [])
        }

        Button {
            visible: root.running
            icon: "stop"
            onClicked: Daemon.command("pomodoro", "stop", [])
        }
    }
}

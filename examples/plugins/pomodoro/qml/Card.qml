import QtQuick
import qs.island
import "Clock.js" as Clock

// The control center's card: the time left and buttons for the timer, or a
// button to start one.
Item {
    id: root

    property var payload: null
    readonly property string phase: payload?.phase ?? "idle"
    readonly property bool running: phase !== "idle"

    implicitHeight: Theme.rowHeight

    Row {
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceMedium

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

            RollingText {
                text: root.running ? Clock.format(root.payload.left) : `${root.payload?.focus_minutes ?? 25} min`
                pixelSize: Theme.textTitle
                weight: Theme.weightTitle
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
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }

    Row {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceSmall

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

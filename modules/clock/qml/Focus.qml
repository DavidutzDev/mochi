import QtQuick
import qs.island

// The Timer tab: the focus timer module's countdown in a ring that waves
// while it runs, the one accent here, with lengths to start focus or a
// break and the controls for what runs. It counts down with the timer
// module's own Countdown view, so both read the time the same way. Without
// the timer module, it says so and opens its settings.
Item {
    id: root

    readonly property bool on: Daemon.modules.includes("timer")
    readonly property var timer: Daemon.state("timer")
    readonly property var countdown: counter.item
    readonly property bool running: countdown?.running ?? false
    readonly property bool paused: countdown?.paused ?? false
    readonly property int sessions: timer?.sessions ?? 0

    Loader {
        id: counter

        active: root.on
        source: "root:/modules/timer/Countdown.qml"
        onLoaded: item.payload = Qt.binding(() => root.timer)
    }

    // The timer module is off.
    Column {
        anchors.centerIn: parent
        width: Math.min(parent.width, 360)
        visible: !root.on
        spacing: Theme.spaceMedium

        Symbol {
            anchors.horizontalCenter: parent.horizontalCenter
            name: "timer_off"
            size: 32
            color: Theme.muted
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: "The focus timer is off"
            color: Theme.foreground
            font.pixelSize: Theme.textTitle
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: "Turn on the timer module to count down focus sessions and breaks from here, with a bubble by the island."
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        ActionButton {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "Open its settings"
            icon: "settings"
            onClicked: Daemon.command("settings", "open", ["timer"])
        }
    }

    WavyRing {
        id: ring

        visible: root.on
        x: Theme.spaceHuge
        anchors.verticalCenter: parent.verticalCenter
        width: 260
        height: 260
        size: 260
        thickness: 10
        value: root.running ? root.countdown.progress : 0
        wavy: root.running && !root.paused
        color: root.paused ? Theme.muted : root.countdown?.resting ? Theme.success : Theme.accent

        Column {
            anchors.centerIn: parent

            RollingText {
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.running ? root.countdown.text : `${root.timer?.focus_minutes ?? 25}:00`
                color: root.running ? Theme.foreground : Theme.muted
                family: Theme.displayFamily
                weight: Theme.weightTitle
                pixelSize: Theme.textDisplay
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: {
                    if (root.running)
                        return root.paused ? `${root.countdown.what}, paused` : root.countdown.what;
                    return "Ready to focus";
                }
                color: Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }
        }
    }

    Column {
        visible: root.on
        anchors.left: ring.right
        anchors.leftMargin: Theme.spaceHuge * 2
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceLarge

        Column {
            width: parent.width
            spacing: Theme.spaceSmall

            SectionLabel {
                text: root.running ? "Start over with focus for" : "Focus for"
            }

            Flow {
                width: parent.width
                spacing: Theme.spaceSmall

                Repeater {
                    model: [15, 25, 45, 60]

                    ActionButton {
                        required property int modelData

                        text: `${modelData} min`
                        onClicked: Daemon.command("timer", "start", [`${modelData}`])
                    }
                }
            }
        }

        Column {
            width: parent.width
            spacing: Theme.spaceSmall

            SectionLabel {
                text: "Or rest for"
            }

            Flow {
                width: parent.width
                spacing: Theme.spaceSmall

                Repeater {
                    model: [5, 10, 15]

                    ActionButton {
                        required property int modelData

                        text: `${modelData} min`
                        icon: "coffee"
                        onClicked: Daemon.command("timer", "break", [`${modelData}`])
                    }
                }
            }
        }

        Row {
            visible: root.running
            spacing: Theme.spaceSmall

            ActionButton {
                text: root.paused ? "Resume" : "Pause"
                icon: root.paused ? "play" : "pause"
                onClicked: Daemon.command("timer", root.paused ? "resume" : "pause", [])
            }

            ActionButton {
                text: "Stop"
                icon: "stop"
                tone: "ghost"
                onClicked: Daemon.command("timer", "stop", [])
            }
        }

        Text {
            text: root.sessions === 0 ? "No sessions done yet" : root.sessions === 1 ? "1 session done" : `${root.sessions} sessions done`
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }
}

import QtQuick
import qs.island
import "Time.js" as Time

// The Timer tab: on the left, the focus timer module's session in a ring
// that waves while it runs, the one accent here, with lengths to start
// focus or a break and the controls for what runs. On the right, its
// custom timers: a field that takes a length the way people type it, like
// 15m Tea, the lengths from `presets`, and each timer with its time left,
// when it ends, "+1 min", pause and stop. Everything counts down with the
// timer module's own Countdown view, so the bubbles and this read the time
// the same way. Without the timer module, it says so and opens its
// settings.
Item {
    id: root

    readonly property bool on: Daemon.modules.includes("timer")
    readonly property var timer: Daemon.state("timer")
    readonly property var countdown: counter.item
    readonly property bool running: countdown?.running ?? false
    readonly property bool paused: countdown?.paused ?? false
    readonly property int sessions: timer?.sessions ?? 0
    readonly property var timers: timer?.timers ?? []
    readonly property bool twelve: (Daemon.state("clock")?.hours ?? "24") === "12"

    // Whether `text` starts with a length the timer reads, like 10m, 1h 30,
    // 90 s or 10:00; the timer itself says what else is wrong.
    function readable(text: string): bool {
        return /^\s*(\d+:\d{1,2}(:\d{1,2})?(\s|$)|\d+(\.\d+)?\s*(h|hrs?|hours?|m|mins?|minutes?|s|secs?|seconds?)?(\s|$|\d))/i.test(text);
    }

    function add(text: string): void {
        if (!readable(text))
            return;
        Daemon.command("timer", "add", [text.trim()]);
        field.text = "";
    }

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
            text: "The timer is off"
            color: Theme.foreground
            font.pixelSize: Theme.textTitle
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: "Turn on the timer module to count down focus sessions, breaks and timers of your own from here, with bubbles by the island."
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

    // The focus session.
    Column {
        id: focusSide

        visible: root.on
        width: 300
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceLarge

        Row {
            spacing: Theme.spaceLarge

            WavyRing {
                id: ring

                width: 128
                height: 128
                size: 128
                thickness: 8
                value: root.running ? root.countdown.progress : 0
                wavy: root.running && !root.paused
                color: root.paused ? Theme.muted : root.countdown?.resting ? Theme.success : Theme.accent

                RollingText {
                    anchors.centerIn: parent
                    text: root.running ? root.countdown.text : `${root.timer?.focus_minutes ?? 25}:00`
                    color: root.running ? Theme.foreground : Theme.muted
                    family: Theme.displayFamily
                    weight: Theme.weightTitle
                    pixelSize: Theme.textHeadline
                }
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: focusSide.width - ring.width - Theme.spaceLarge
                spacing: Theme.spaceSmall

                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    text: {
                        if (root.running)
                            return root.paused ? `${root.countdown.what}, paused` : root.countdown.what;
                        return "Ready to focus";
                    }
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    text: root.sessions === 0 ? "No sessions done yet" : root.sessions === 1 ? "1 session done" : `${root.sessions} sessions done`
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
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
                        icon: "stop"
                        tone: "ghost"
                        onClicked: Daemon.command("timer", "stop", [])
                    }
                }
            }
        }

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
    }

    // A line between the session and the timers.
    Rectangle {
        id: rule

        visible: root.on
        x: focusSide.width + Theme.spaceLarge
        width: 1
        height: parent.height
        color: Theme.border
    }

    // The custom timers.
    Column {
        id: timersSide

        visible: root.on
        anchors.left: rule.right
        anchors.leftMargin: Theme.spaceLarge
        anchors.right: parent.right
        height: parent.height
        spacing: Theme.spaceSmall

        SectionLabel {
            text: "Timers"
        }

        Row {
            width: parent.width
            spacing: Theme.spaceSmall

            Field {
                id: field

                width: parent.width - startButton.width - Theme.spaceSmall
                placeholder: "15m Tea, 90s, 1h 30m, 10:00"
                maximumLength: 80
                onAccepted: root.add(text)
            }

            ActionButton {
                id: startButton

                anchors.verticalCenter: parent.verticalCenter
                enabled: root.readable(field.text)
                text: "Start"
                icon: "play"
                onClicked: root.add(field.text)
            }
        }

        // What's wrong with what's typed, before Start.
        Text {
            visible: field.text.trim() !== "" && !root.readable(field.text)
            width: parent.width
            wrapMode: Text.Wrap
            text: "Start with a length, like 10m, 90s, 1h 30m or 10:00; a label can follow it."
            color: Theme.danger
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Flow {
            width: parent.width
            spacing: Theme.spaceTiny

            Repeater {
                model: root.timer?.presets ?? []

                ActionButton {
                    required property var modelData

                    text: modelData.label
                    tone: "ghost"
                    onClicked: Daemon.command("timer", "add", [modelData.text])
                }
            }
        }

        // No timers yet.
        Text {
            visible: root.timers.length === 0
            width: parent.width
            topPadding: Theme.spaceMedium
            wrapMode: Text.Wrap
            text: "No timers running. Type a length above, or :t 10m Pizza in the launcher; several can run at once."
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        ListView {
            id: list

            visible: root.timers.length > 0
            width: parent.width
            height: parent.height - y
            clip: true
            spacing: Theme.spaceTiny
            boundsBehavior: Flickable.StopAtBounds
            model: root.timers

            ScrollFade {
                view: list
            }

            delegate: Item {
                id: row

                required property var modelData

                readonly property var count: rowCounter.item
                readonly property bool paused: row.modelData.paused === true

                width: list.width
                height: 48

                Loader {
                    id: rowCounter

                    source: "root:/modules/timer/Countdown.qml"
                    onLoaded: item.payload = Qt.binding(() => row.modelData)
                }

                WavyRing {
                    id: dial

                    anchors.verticalCenter: parent.verticalCenter
                    width: 32
                    height: 32
                    size: 32
                    thickness: 3
                    value: row.count?.progress ?? 0
                    wavy: false
                    color: row.paused ? Theme.muted : Theme.foreground
                }

                Column {
                    anchors.left: dial.right
                    anchors.leftMargin: Theme.spaceMedium
                    anchors.right: buttons.left
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter

                    RollingText {
                        text: row.count?.text ?? ""
                        color: row.paused ? Theme.muted : Theme.foreground
                        pixelSize: Theme.textTitle
                        weight: Theme.weightTitle
                    }

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: {
                            const name = row.modelData.name ?? "";
                            if (row.paused)
                                return `${name}, paused`;
                            const ends = new Date(row.modelData.ends_ms ?? Date.now());
                            return `${name} · ends ${Time.time(ends, root.twelve)}`;
                        }
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }
                }

                Row {
                    id: buttons

                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.spaceTiny

                    ActionButton {
                        text: root.timer?.more_label ?? "+1 min"
                        tone: "ghost"
                        onClicked: Daemon.command("timer", "extend", [`${row.modelData.id}`])
                    }

                    ActionButton {
                        icon: row.paused ? "play" : "pause"
                        onClicked: Daemon.command("timer", row.paused ? "resume" : "pause", [`${row.modelData.id}`])
                    }

                    ActionButton {
                        icon: "stop"
                        tone: "ghost"
                        onClicked: Daemon.command("timer", "stop", [`${row.modelData.id}`])
                    }
                }
            }
        }
    }
}

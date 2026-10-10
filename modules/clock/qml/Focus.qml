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
// the same way. Along the bottom, the alarm: its volume, its sound with
// Choose and the theme's default, and Play it; each change goes to the
// timer's settings through the settings module and applies at once.
// Without the timer module, it says so and opens its settings.
Item {
    id: root

    readonly property bool on: Daemon.modules.includes("timer")
    readonly property var timer: Daemon.state("timer")
    readonly property var alarm: timer?.alarm ?? null
    // The alarm's settings are changed through the settings module.
    readonly property bool settingsOn: Daemon.modules.includes("settings")
    readonly property string soundPath: "config.module.timer.sound_file"
    readonly property bool choosing: Daemon.state("settings")?.choosing === soundPath
    // What the settings module last said about the sound file, like a
    // chooser that couldn't open.
    readonly property string soundError: {
        const error = Daemon.state("settings")?.error;
        return error?.path === soundPath ? error.message : "";
    }
    // The error closed here, which stays closed until another comes.
    property string dismissedError: ""
    readonly property bool showError: soundError !== "" && soundError !== dismissedError && !choosing
    // The volume on the way while the slider moves, or -1.
    property int pendingVolume: -1
    readonly property int volume: pendingVolume >= 0 ? pendingVolume : alarm?.volume ?? 80

    function setVolume(volume: int): void {
        pendingVolume = Math.max(0, Math.min(100, volume));
        settle.restart();
    }

    // The sound file's name without its folder or its ending, like "Bell".
    function soundName(file: string): string {
        if (file === "")
            return "The theme's alarm";
        const name = file.split("/").pop();
        const dot = name.lastIndexOf(".");
        return dot > 0 ? name.slice(0, dot) : name;
    }

    // A drag writes the volume once it rests, not at every step.
    Timer {
        id: settle

        interval: 300
        onTriggered: {
            Daemon.command("settings", "set", ["config.module.timer.volume", `${root.pendingVolume}`]);
            forget.restart();
        }
    }

    // A volume the settings refused goes back to the timer's.
    Timer {
        id: forget

        interval: 2000
        onTriggered: {
            if (!settle.running)
                root.pendingVolume = -1;
        }
    }

    // The timer's answer replaces what was on the way.
    onAlarmChanged: {
        if (!settle.running && alarm?.volume === pendingVolume)
            pendingVolume = -1;
    }
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

    // The room above the alarm.
    Item {
        id: top

        visible: root.on
        width: parent.width
        height: parent.height - alarmRow.height - Theme.spaceMedium
    }

    // The focus session.
    Column {
        id: focusSide

        visible: root.on
        width: 300
        anchors.verticalCenter: top.verticalCenter
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
        height: top.height
        color: Theme.border
    }

    // The custom timers.
    Column {
        id: timersSide

        visible: root.on
        anchors.left: rule.right
        anchors.leftMargin: Theme.spaceLarge
        anchors.right: parent.right
        height: top.height
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

            WheelScroll {
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

    // The alarm, for every timer: how loud, which sound, and a button to
    // hear it. The slider moves with the keyboard too, 5% a press.
    Rectangle {
        id: alarmRow

        visible: root.on
        anchors.bottom: parent.bottom
        width: parent.width
        height: Theme.rowHeight
        radius: Theme.radiusField
        color: Theme.surface

        // What went wrong choosing a sound, like no file chooser, over the
        // volume and the sound's name until it's closed or the next
        // change clears it.
        Row {
            visible: root.showError
            x: Theme.spaceMedium
            width: sound.x - x - Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: "error"
                size: 18
                color: Theme.danger
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - Theme.spaceSmall * 2 - 18 - closeError.width
                text: root.soundError
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
                color: Theme.foreground
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            ActionButton {
                id: closeError

                anchors.verticalCenter: parent.verticalCenter
                icon: "close"
                tone: "ghost"
                onClicked: root.dismissedError = root.soundError
            }
        }

        Row {
            id: level

            visible: !root.showError
            x: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: root.volume === 0 ? "notifications_off" : "alarm"
                size: 18
                color: Theme.muted
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Alarm"
                color: Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }

            // The player of `sound_command` keeps its own volume.
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.alarm?.own_command === true
                text: "Your sound_command sets the volume"
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            Item {
                id: volumeKeys

                anchors.verticalCenter: parent.verticalCenter
                visible: root.alarm?.own_command !== true
                width: 132
                height: Theme.controlHeight
                enabled: root.settingsOn
                opacity: enabled ? 1 : 0.4
                activeFocusOnTab: enabled
                Keys.onLeftPressed: root.setVolume(root.volume - 5)
                Keys.onRightPressed: root.setVolume(root.volume + 5)

                Slider {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    thickness: 4
                    value: root.volume / 100
                    // A double click goes back to the default, 80%.
                    reset: 0.8
                    onMoved: value => root.setVolume(Math.round(value * 100))
                    onReleased: value => root.setVolume(Math.round(value * 100))
                }

                Rectangle {
                    visible: volumeKeys.activeFocus
                    anchors.fill: parent
                    anchors.margins: -3
                    radius: height / 2
                    color: "transparent"
                    border.width: 2
                    border.color: Theme.accent
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.alarm?.own_command !== true
                width: 36
                text: `${root.volume}%`
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        Rectangle {
            id: divider

            visible: !root.showError
            anchors.left: level.right
            anchors.leftMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: Theme.controlHeight - Theme.spaceSmall
            color: Theme.border
        }

        Symbol {
            id: note

            visible: !root.showError
            anchors.left: divider.right
            anchors.leftMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            name: "music_note"
            size: 18
            color: Theme.muted
        }

        // The sound, while a chooser is open, or its name.
        Text {
            visible: !root.showError
            anchors.left: note.right
            anchors.leftMargin: Theme.spaceSmall
            anchors.right: sound.left
            anchors.rightMargin: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideRight
            text: root.choosing ? "Choosing a sound…" : root.soundName(root.alarm?.sound_file ?? "")
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Row {
            id: sound

            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceTiny

            ActionButton {
                visible: root.settingsOn
                text: "Choose…"
                icon: "folder_open"
                tone: "ghost"
                onClicked: Daemon.command("settings", "choose-file", [root.soundPath])
            }

            // Back to alarm-clock-elapsed from the sound theme.
            ActionButton {
                visible: root.settingsOn && (root.alarm?.sound_file ?? "") !== ""
                text: "Default"
                icon: "restart_alt"
                tone: "ghost"
                onClicked: Daemon.command("settings", "set", [root.soundPath, "\"\""])
            }

            ActionButton {
                enabled: root.volume > 0
                text: "Play it"
                icon: "play"
                onClicked: Daemon.command("timer", "test-sound", [])
            }
        }
    }
}

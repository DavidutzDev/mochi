import QtQuick
import qs.island
import "Time.js" as Time

// The Today tab: the time big inside a shape in the accent color, the one
// accent here, a cookie unless the `shape` setting picks another or none,
// with the seconds under the minutes if `seconds` is on; the date above,
// with how far through the day it is on a thin line, or as a ring around
// the shape, and a greeting under. Beside it, the weather now, its details,
// the next hours and days; and the next reminder along the bottom, or the
// oldest one still due.
Item {
    id: root

    // The clock module's state, and the weather module's.
    property var clock: ({})
    property var weather: null
    property bool weatherOn: false
    signal addReminder

    readonly property bool twelve: clock.hours === "12"
    readonly property bool seconds: clock.seconds === true
    readonly property string progress: clock.day_progress ?? "line"
    readonly property string shape: clock.shape ?? "cookie"
    // How far through the day it is, from 0 to 1.
    readonly property real dayDone: (here.parts.hours * 3600 + here.parts.minutes * 60 + here.parts.seconds) / 86400
    // The first reminder not done, by when it's due: one that's due comes
    // before the ones ahead.
    readonly property var next: {
        const open = (clock.reminders ?? []).filter(reminder => !reminder.done);
        const due = reminder => reminder.snoozed ?? reminder.at;
        return open.sort((a, b) => due(a) - due(b))[0] ?? null;
    }

    ClockTime {
        id: here

        seconds: root.seconds
    }

    readonly property string greeting: {
        const hour = here.parts.hours;
        if (hour >= 5 && hour < 12)
            return "Good morning";
        if (hour >= 12 && hour < 18)
            return "Good afternoon";
        if (hour >= 18)
            return "Good evening";
        return "Good night";
    }

    // Under the time and the weather.
    readonly property int strip: Theme.rowHeight + Theme.spaceLarge

    // The time.
    Rectangle {
        id: timeCard

        width: 216
        height: parent.height - root.strip - Theme.spaceMedium
        radius: Theme.radiusSurface
        color: Theme.surface

        Column {
            id: dateLines

            x: Theme.spaceLarge
            y: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: Qt.locale().dayName(here.parts.day, Locale.LongFormat)
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: `${here.parts.date} ${Qt.locale().monthName(here.parts.month, Locale.LongFormat)}`
                color: Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }

            // How far through the day, with the share beside it.
            Item {
                visible: root.progress === "line"
                width: parent.width
                height: visible ? percent.implicitHeight + Theme.spaceSmall : 0

                ProgressBar {
                    anchors.left: parent.left
                    anchors.right: percent.left
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: percent.verticalCenter
                    height: 3
                    value: root.dayDone
                }

                Text {
                    id: percent

                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    text: `${Math.floor(root.dayDone * 100)}%`
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.features: {
                        "tnum": 1
                    }
                }
            }
        }

        // The room between the date and the greeting, square.
        Item {
            id: slot

            readonly property real from: dateLines.y + dateLines.height + Theme.spaceMedium
            readonly property real to: greetingLine.y - Theme.spaceMedium

            anchors.horizontalCenter: parent.horizontalCenter
            y: from + (to - from - height) / 2
            width: Math.min(parent.width - Theme.spaceLarge * 2, to - from)
            height: width

            // The day so far, around the shape, from the top.
            WavyRing {
                visible: root.progress === "ring"
                anchors.fill: parent
                size: slot.width
                thickness: 4
                wavy: false
                color: Theme.foreground
                value: root.dayDone
            }

            ExpressiveShape {
                id: cookie

                readonly property bool none: root.shape === "none"
                readonly property color ink: none ? Theme.foreground : Theme.onAccent
                // A wide room, like a pill's, takes the time on one line.
                readonly property bool oneLine: roomWidth > roomHeight * 1.4

                anchors.centerIn: parent
                width: root.progress === "ring" ? slot.width - 20 : slot.width
                height: width
                // Without a shape, the time takes a squircle's room.
                shape: none ? "squircle" : root.shape
                color: none ? "transparent" : Theme.accent

                // The seconds under the minutes, under half their size.
                Column {
                    anchors.centerIn: parent
                    visible: !cookie.oneLine

                    StackedTime {
                        id: stacked

                        anchors.horizontalCenter: parent.horizontalCenter
                        width: cookie.roomWidth * 0.78
                        height: cookie.roomHeight * (root.seconds ? 0.7 : 0.94)
                        hours: here.hour(root.twelve)
                        minutes: here.pad(here.parts.minutes)
                        color: cookie.ink
                    }

                    RollingText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        visible: root.seconds
                        text: here.pad(here.parts.seconds)
                        color: cookie.none ? Theme.muted : Qt.alpha(Theme.onAccent, 0.75)
                        family: Theme.displayFamily
                        weight: Theme.weightTitle
                        pixelSize: Math.max(Theme.textCaption, stacked.pixelSize * 0.42)
                    }
                }

                RollingText {
                    anchors.centerIn: parent
                    visible: cookie.oneLine
                    text: here.time(root.twelve, root.seconds)
                    color: cookie.ink
                    family: Theme.displayFamily
                    weight: Theme.weightTitle
                    // A digit is about 0.62 of the size wide.
                    pixelSize: Math.max(Theme.textCaption, Math.min(cookie.roomHeight * 0.7, cookie.roomWidth / (text.length * 0.62)))
                }
            }
        }

        Text {
            id: greetingLine

            x: Theme.spaceLarge
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2
            elide: Text.ElideRight
            text: root.twelve ? `${here.half} · ${root.greeting}` : root.greeting
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightLabel
        }
    }

    TodayWeather {
        anchors.left: timeCard.right
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: parent.right
        height: timeCard.height
        weather: root.weather
        on: root.weatherOn
        twelve: root.twelve
    }

    // The next reminder, and adding one.
    Rectangle {
        id: reminder

        anchors.bottom: parent.bottom
        width: parent.width
        height: root.strip
        radius: Theme.radiusSurface
        color: Theme.surface

        readonly property date when: new Date((root.next ? root.next.snoozed ?? root.next.at : 0) * 1000)
        readonly property bool due: root.next !== null && when <= here.now

        Rectangle {
            id: bell

            x: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.controlHeight
            height: width
            radius: width / 2
            color: Theme.raised

            Symbol {
                anchors.centerIn: parent
                name: "notifications"
                size: 18
                color: reminder.due ? Theme.foreground : Theme.muted
                filled: reminder.due
            }
        }

        Column {
            anchors.left: bell.right
            anchors.leftMargin: Theme.spaceMedium
            anchors.right: actions.left
            anchors.rightMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: root.next ? root.next.text : "No reminders coming up"
                color: root.next ? Theme.foreground : Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                font.weight: root.next ? Theme.weightLabel : Theme.weightBody
            }

            Text {
                width: parent.width
                visible: root.next !== null
                elide: Text.ElideRight
                text: {
                    if (!root.next)
                        return "";
                    const at = new Date(root.next.at * 1000);
                    const time = Time.time(at, root.twelve);
                    const day = Time.sameDay(at, here.now) ? time : `${Time.shortDate(at)}, ${time}`;
                    if (reminder.due)
                        return `Due ${Time.relative(at, here.now)} · ${day}`;
                    if (root.next.snoozed != null)
                        return `${day} · snoozed, back ${Time.relative(reminder.when, here.now)}`;
                    return `${Time.sameDay(at, here.now) ? `Today, ${time}` : day} · ${Time.relative(at, here.now)}`;
                }
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        Row {
            id: actions

            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceSmall

            ActionButton {
                visible: reminder.due
                text: "Done"
                icon: "check"
                onClicked: Daemon.command("clock", "done", [`${root.next.id}`])
            }

            ActionButton {
                text: "Add a reminder"
                icon: "add"
                onClicked: root.addReminder()
            }
        }
    }
}

import QtQuick
import qs.island
import "Time.js" as Time

// The Today tab: the time big inside a cookie in the accent color, the one
// accent here, with the date above and a greeting under; the weather now,
// its details, the next hours and days beside it; and the next reminder
// along the bottom, or the oldest one still due.
Item {
    id: root

    // The clock module's state, and the weather module's.
    property var clock: ({})
    property var weather: null
    property bool weatherOn: false
    signal addReminder

    readonly property bool twelve: clock.hours === "12"
    // The first reminder not done, by when it's due: one that's due comes
    // before the ones ahead.
    readonly property var next: {
        const open = (clock.reminders ?? []).filter(reminder => !reminder.done);
        const due = reminder => reminder.snoozed ?? reminder.at;
        return open.sort((a, b) => due(a) - due(b))[0] ?? null;
    }

    ClockTime {
        id: here
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
        }

        ExpressiveShape {
            id: cookie

            anchors.horizontalCenter: parent.horizontalCenter
            y: dateLines.y + dateLines.height + (parent.height - dateLines.y - dateLines.height - greetingLine.height - Theme.spaceLarge - size) / 2
            size: Math.min(parent.width - Theme.spaceLarge * 2, parent.height - dateLines.height - greetingLine.height - Theme.spaceLarge * 2 - Theme.spaceMedium * 2)
            shape: "cookie"
            color: Theme.accent

            StackedTime {
                anchors.centerIn: parent
                width: cookie.size * 0.5
                height: cookie.size * 0.62
                hours: here.hour(root.twelve)
                minutes: here.pad(here.parts.minutes)
                color: Theme.onAccent
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

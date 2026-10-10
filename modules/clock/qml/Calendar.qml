import QtQuick
import qs.island
import "Time.js" as Time

// The Calendar tab: a month on the left, today in an accent pentagon as on the
// calendar widgets, the one accent here, and a dot under each day with
// reminders; the picked day's reminders on the right, with a field to add one.
// The arrows move the picked day while the month has the keyboard, Page Up and
// Page Down turn the month, and Home goes back to today.
Item {
    id: root

    property var clock: ({})
    // Built from Today's "Add a reminder": the time field takes the
    // keyboard.
    property bool adding: false

    readonly property bool twelve: clock.hours === "12"
    readonly property bool sundayFirst: clock.first_day === "sunday"

    ClockTime {
        id: here
    }

    readonly property date today: Time.day(here.now)
    property date picked: Time.day(new Date())
    // The month shown is the picked day's.
    readonly property int year: picked.getFullYear()
    readonly property int month: picked.getMonth()
    // The first day on the grid: the start of the week the month starts in.
    readonly property date first: {
        const start = new Date(year, month, 1);
        const back = (start.getDay() - (sundayFirst ? 0 : 1) + 7) % 7;
        return new Date(year, month, 1 - back);
    }

    // The reminders by day, as "2026-10-21", each list by time.
    readonly property var byDay: {
        const days = {};
        for (const reminder of clock.reminders ?? []) {
            const key = Time.isoDate(new Date(reminder.at * 1000));
            (days[key] = days[key] ?? []).push(reminder);
        }
        return days;
    }
    readonly property var dayReminders: (byDay[Time.isoDate(picked)] ?? []).slice().sort((a, b) => a.at - b.at)
    readonly property bool past: picked < today

    function pick(date: date): void {
        picked = Time.day(date);
    }

    // The same day in another month, or its last day when that's shorter.
    function turn(months: int): void {
        const last = new Date(year, month + months + 1, 0).getDate();
        pick(new Date(year, month + months, Math.min(picked.getDate(), last)));
    }

    // What the time field starts with: the next hour today, nine in the
    // morning another day.
    function suggested(): string {
        let hour = 9;
        if (Time.sameDay(picked, here.now))
            hour = Math.min(23, here.now.getHours() + 1);
        return Time.time(new Date(2000, 0, 1, hour, 0), twelve);
    }

    property string problem: ""

    function add(): void {
        const parts = Time.parseTime(time.text);
        if (parts === null) {
            problem = twelve ? "A time like 6:30 PM" : "A time like 18:30";
            time.focusInput();
            return;
        }
        const at = new Date(picked.getFullYear(), picked.getMonth(), picked.getDate(), parts[0], parts[1]);
        if (at < here.now - 60000) {
            problem = "That time has passed";
            time.focusInput();
            return;
        }
        if (what.text.trim() === "") {
            problem = "Write what to remind you of";
            what.focusInput();
            return;
        }
        Daemon.command("clock", "remind", [Time.isoDate(at), `${Time.pad(parts[0])}:${Time.pad(parts[1])}`, what.text.trim()]);
        what.text = "";
        problem = "";
    }

    onPickedChanged: {
        time.text = suggested();
        problem = "";
    }
    Component.onCompleted: {
        time.text = suggested();
        if (adding)
            Qt.callLater(() => what.focusInput());
    }

    // The month.
    Item {
        id: monthView

        width: 300
        height: parent.height

        Item {
            id: header

            width: parent.width
            height: Theme.controlHeight

            Text {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: `${Qt.locale().standaloneMonthName(root.month, Locale.LongFormat)} ${root.year}`
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceTiny

                ActionButton {
                    visible: root.year !== root.today.getFullYear() || root.month !== root.today.getMonth()
                    text: "Today"
                    tone: "ghost"
                    onClicked: root.pick(root.today)
                }

                ActionButton {
                    icon: "chevron_left"
                    tone: "ghost"
                    onClicked: root.turn(-1)
                }

                ActionButton {
                    icon: "chevron_right"
                    tone: "ghost"
                    onClicked: root.turn(1)
                }
            }
        }

        // The weekdays' initials.
        Row {
            id: weekdays

            anchors.top: header.bottom
            anchors.topMargin: Theme.spaceMedium

            Repeater {
                model: 7

                Text {
                    required property int index

                    width: monthView.width / 7
                    horizontalAlignment: Text.AlignHCenter
                    text: Qt.locale().dayName((index + (root.sundayFirst ? 0 : 1)) % 7, Locale.NarrowFormat)
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                }
            }
        }

        // Six weeks, which every month fits in. It takes the keyboard as
        // one stop, and the arrows move inside it.
        Grid {
            id: grid

            anchors.top: weekdays.bottom
            anchors.topMargin: Theme.spaceSmall
            columns: 7
            activeFocusOnTab: true
            Keys.onLeftPressed: root.pick(new Date(root.year, root.month, root.picked.getDate() - 1))
            Keys.onRightPressed: root.pick(new Date(root.year, root.month, root.picked.getDate() + 1))
            Keys.onUpPressed: root.pick(new Date(root.year, root.month, root.picked.getDate() - 7))
            Keys.onDownPressed: root.pick(new Date(root.year, root.month, root.picked.getDate() + 7))
            Keys.onPressed: event => {
                if (event.key === Qt.Key_PageUp)
                    root.turn(-1);
                else if (event.key === Qt.Key_PageDown)
                    root.turn(1);
                else if (event.key === Qt.Key_Home)
                    root.pick(root.today);
                else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                    time.focusInput();
                else
                    return;
                event.accepted = true;
            }

            Repeater {
                model: 42

                Item {
                    id: cell

                    required property int index
                    readonly property date date: new Date(root.first.getFullYear(), root.first.getMonth(), root.first.getDate() + index)
                    readonly property bool inMonth: date.getMonth() === root.month
                    readonly property bool isToday: Time.sameDay(date, root.today)
                    readonly property bool isPicked: Time.sameDay(date, root.picked)
                    readonly property var reminders: root.byDay[Time.isoDate(date)] ?? []
                    readonly property bool open: reminders.some(reminder => !reminder.done)

                    width: monthView.width / 7
                    // Six weeks fill what's left under the weekdays.
                    height: Math.floor((monthView.height - grid.y) / 6)

                    // The picked day, with a ring while the month has the
                    // keyboard.
                    Rectangle {
                        anchors.centerIn: parent
                        width: 34
                        height: 34
                        radius: height / 2
                        visible: cell.isPicked
                        color: Theme.raised
                        border.width: grid.activeFocus ? 2 : 0
                        border.color: Theme.accent
                    }

                    ExpressiveShape {
                        anchors.centerIn: parent
                        visible: cell.isToday
                        shape: "pentagon"
                        size: 28
                        color: Theme.accent
                    }

                    Text {
                        anchors.centerIn: parent
                        anchors.verticalCenterOffset: cell.reminders.length > 0 ? -2 : 0
                        text: cell.date.getDate()
                        color: cell.isToday ? Theme.onAccent : cell.inMonth ? Theme.foreground : Theme.muted
                        opacity: cell.inMonth || cell.isToday ? 1 : 0.6
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                        font.weight: cell.isToday || cell.isPicked ? Theme.weightTitle : Theme.weightBody
                        font.features: {
                            "tnum": 1
                        }
                    }

                    // A day with reminders: full while one isn't done.
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: 9
                        visible: cell.reminders.length > 0
                        width: 4
                        height: 4
                        radius: 2
                        color: cell.isToday ? Theme.onAccent : cell.open ? Theme.foreground : Theme.muted
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.pick(cell.date)
                    }
                }
            }
        }
    }

    // The picked day.
    Rectangle {
        id: dayView

        anchors.left: monthView.right
        anchors.leftMargin: Theme.spaceLarge
        anchors.right: parent.right
        height: parent.height
        radius: Theme.radiusSurface
        color: Theme.surface

        Column {
            id: dayHeading

            x: Theme.spaceLarge
            y: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: Time.longDate(root.picked)
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: {
                    const count = root.dayReminders.length;
                    const reminders = count === 0 ? "No reminders" : count === 1 ? "1 reminder" : `${count} reminders`;
                    const days = Time.daysBetween(root.today, root.picked);
                    const when = days === 0 ? "Today" : days === 1 ? "Tomorrow" : days === -1 ? "Yesterday" : days > 0 ? `In ${days} days` : `${-days} days ago`;
                    return `${when} · ${reminders}`;
                }
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        ListView {
            id: list

            anchors.top: dayHeading.bottom
            anchors.topMargin: Theme.spaceMedium
            anchors.bottom: form.top
            anchors.bottomMargin: Theme.spaceMedium
            x: Theme.spaceSmall
            width: parent.width - Theme.spaceSmall * 2
            clip: true
            spacing: 2
            boundsBehavior: Flickable.StopAtBounds
            model: root.dayReminders

            ScrollFade {
                view: list
                color: Theme.surface
            }

            WheelScroll {
                view: list
            }

            delegate: Item {
                id: row

                required property var modelData
                readonly property bool due: !modelData.done && (modelData.snoozed ?? modelData.at) * 1000 <= here.now.getTime()

                width: list.width
                height: Theme.rowHeight

                Text {
                    id: at

                    x: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter
                    width: root.twelve ? 64 : 44
                    text: Time.time(new Date(row.modelData.at * 1000), root.twelve)
                    color: row.modelData.done ? Theme.muted : Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                    font.features: {
                        "tnum": 1
                    }
                }

                Column {
                    anchors.left: at.right
                    anchors.leftMargin: Theme.spaceSmall
                    anchors.right: buttons.left
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: row.modelData.text
                        color: row.modelData.done ? Theme.muted : Theme.foreground
                        font.strikeout: row.modelData.done
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }

                    Text {
                        visible: row.due || row.modelData.done || row.modelData.snoozed != null
                        text: row.modelData.done ? "Done" : row.due ? "Due" : `Snoozed until ${Time.time(new Date(row.modelData.snoozed * 1000), root.twelve)}`
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
                        visible: row.due
                        icon: "check"
                        onClicked: Daemon.command("clock", "done", [`${row.modelData.id}`])
                    }

                    ActionButton {
                        icon: "delete"
                        tone: "ghost"
                        onClicked: Daemon.command("clock", "delete", [`${row.modelData.id}`])
                    }
                }
            }
        }

        Text {
            anchors.centerIn: list
            width: list.width - Theme.spaceLarge * 2
            visible: root.dayReminders.length === 0
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: root.past ? "Nothing was set for this day." : "Nothing on this day yet. Add a reminder below."
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        // Adding one: a time, what, and Add. Days gone by take none.
        Column {
            id: form

            x: Theme.spaceLarge
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2
            spacing: Theme.spaceSmall

            Row {
                width: parent.width
                spacing: Theme.spaceSmall
                visible: !root.past

                Field {
                    id: time

                    width: root.twelve ? 92 : 64
                    placeholder: root.twelve ? "6:30 PM" : "18:30"
                    maximumLength: 8
                    onAccepted: what.focusInput()
                }

                Field {
                    id: what

                    width: parent.width - time.width - addButton.width - Theme.spaceSmall * 2
                    placeholder: "What to remind you of"
                    maximumLength: 200
                    onAccepted: root.add()
                }

                ActionButton {
                    id: addButton

                    anchors.verticalCenter: parent.verticalCenter
                    text: "Add"
                    icon: "add"
                    onClicked: root.add()
                }
            }

            Text {
                width: parent.width
                visible: root.problem !== "" || root.past
                text: root.past ? "This day has gone by." : root.problem
                color: root.past ? Theme.muted : Theme.danger
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }
}

import QtQuick
import qs.island
import "Time.js" as Time

// The control center's Today card: the time, large, and beside it the
// weekday and the date; the stopwatch instead while it has time on it, with
// a button to pause or resume it; or a reminder due within a day. Its
// heading opens the Clock page.
Item {
    id: root

    property var payload: null
    readonly property var clock: payload ?? ({})
    readonly property bool twelve: clock.hours === "12"
    readonly property var watch: clock.stopwatch ?? ({})
    readonly property bool running: watch.since_ms != null
    readonly property bool hundredths: clock.precision === "hundredths"

    property real now: Date.now()
    readonly property real elapsed: (watch.banked_ms ?? 0) + (running ? Math.max(0, now - watch.since_ms) : 0)
    readonly property bool timing: elapsed > 0

    // The first reminder not done, by when it's due, as on Today, when it's
    // due within a day or already is: the date shows otherwise.
    readonly property var next: {
        const open = (clock.reminders ?? []).filter(reminder => !reminder.done);
        const due = reminder => reminder.snoozed ?? reminder.at;
        const first = open.sort((a, b) => due(a) - due(b))[0] ?? null;
        return first && due(first) * 1000 - here.now.getTime() < 24 * 3600 * 1000 ? first : null;
    }

    implicitHeight: Theme.rowHeight

    Timer {
        interval: 100
        repeat: true
        running: root.running && root.visible
        onTriggered: root.now = Date.now()
    }
    onWatchChanged: now = Date.now()

    ClockTime {
        id: here
    }

    function pad(value: int): string {
        return `${value}`.padStart(2, "0");
    }

    // As the Stopwatch tab writes it.
    function format(ms: real): string {
        const fraction = hundredths ? pad(Math.floor(ms / 10) % 100) : `${Math.floor(ms / 100) % 10}`;
        const seconds = Math.floor(ms / 1000);
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor(seconds / 60) % 60;
        const rest = `${pad(seconds % 60)}.${fraction}`;
        return hours > 0 ? `${hours}:${pad(minutes)}:${rest}` : `${minutes}:${rest}`;
    }

    RollingText {
        id: time

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        text: here.time(root.twelve, false)
        pixelSize: Theme.textDisplay
        family: Theme.displayFamily
        weight: Theme.weightTitle
    }

    Column {
        anchors.left: time.right
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: toggle.visible ? toggle.left : parent.right
        anchors.rightMargin: toggle.visible ? Theme.spaceSmall : 0
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: {
                if (root.timing)
                    return root.format(root.elapsed);
                return root.next ? root.next.text : here.now.toLocaleDateString(Qt.locale(), "dddd");
            }
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightLabel
            font.features: {
                "tnum": 1
            }
        }

        Text {
            width: parent.width
            visible: text !== ""
            elide: Text.ElideRight
            text: {
                if (root.timing)
                    return root.running ? "Stopwatch" : "Stopwatch, paused";
                if (!root.next)
                    return here.now.toLocaleDateString(Qt.locale(), "d MMMM");
                const at = new Date((root.next.snoozed ?? root.next.at) * 1000);
                if (at <= here.now)
                    return `Due ${Time.relative(at, here.now)}`;
                const clockTime = Time.time(at, root.twelve);
                return Time.sameDay(at, here.now) ? `${clockTime} · ${Time.relative(at, here.now)}` : `${Time.shortDate(at)}, ${clockTime}`;
            }
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }

    ActionButton {
        id: toggle

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        visible: root.timing
        icon: root.running ? "pause" : "play"
        tone: "ghost"
        onClicked: Daemon.command("clock", "stopwatch", [root.running ? "pause" : "start"])
    }
}

import QtQuick
import qs.island

// The Stopwatch tab: the time big, with Start or Pause in the accent color,
// the one accent here, Lap and Reset under it; the laps on the right,
// newest first, with how long each took and the fastest and slowest named.
// The clock module keeps the stopwatch, so it runs on with the panel
// closed; this counts up from when it started.
Item {
    id: root

    property var clock: ({})
    readonly property var watch: clock.stopwatch ?? ({})
    readonly property bool running: watch.since_ms != null
    readonly property var laps: watch.laps ?? []

    property real now: Date.now()
    readonly property real elapsed: (watch.banked_ms ?? 0) + (running ? Math.max(0, now - watch.since_ms) : 0)

    Timer {
        interval: 50
        repeat: true
        running: root.running && root.visible
        onTriggered: root.now = Date.now()
    }
    onWatchChanged: now = Date.now()

    function pad(value: int): string {
        return `${value}`.padStart(2, "0");
    }

    // m:ss.cc, or h:mm:ss.cc from an hour.
    function format(ms: real): string {
        const hundredths = Math.floor(ms / 10) % 100;
        const seconds = Math.floor(ms / 1000);
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor(seconds / 60) % 60;
        const rest = `${pad(seconds % 60)}.${pad(hundredths)}`;
        return hours > 0 ? `${hours}:${pad(minutes)}:${rest}` : `${minutes}:${rest}`;
    }

    // Each lap with how long it took, newest first.
    readonly property var rows: {
        const rows = [];
        let before = 0;
        root.laps.forEach((total, index) => {
            rows.push({
                "number": index + 1,
                "split": total - before,
                "total": total
            });
            before = total;
        });
        return rows.reverse();
    }
    readonly property real fastest: rows.length > 1 ? Math.min(...rows.map(row => row.split)) : -1
    readonly property real slowest: rows.length > 1 ? Math.max(...rows.map(row => row.split)) : -1

    Column {
        id: face

        width: parent.width * 0.5
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceHuge

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.format(root.elapsed)
            color: root.running || root.elapsed > 0 ? Theme.foreground : Theme.muted
            font.pixelSize: Theme.textDisplay * 1.4
            font.family: Theme.displayFamily
            font.weight: Theme.weightTitle
            font.features: {
                "tnum": 1
            }
        }

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: Theme.spaceSmall

            ActionButton {
                text: root.running ? "Pause" : root.elapsed > 0 ? "Resume" : "Start"
                icon: root.running ? "pause" : "play"
                tone: "accent"
                onClicked: Daemon.command("clock", "stopwatch", [root.running ? "pause" : "start"])
            }

            ActionButton {
                enabled: root.running
                text: "Lap"
                icon: "flag"
                onClicked: Daemon.command("clock", "stopwatch", ["lap"])
            }

            ActionButton {
                enabled: !root.running && root.elapsed > 0
                text: "Reset"
                icon: "restart_alt"
                tone: "ghost"
                onClicked: Daemon.command("clock", "stopwatch", ["reset"])
            }
        }
    }

    Rectangle {
        anchors.left: face.right
        anchors.right: parent.right
        height: parent.height
        radius: Theme.radiusSurface
        color: Theme.surface

        Text {
            id: lapsTitle

            x: Theme.spaceLarge
            y: Theme.spaceLarge
            text: root.laps.length === 0 ? "Laps" : root.laps.length === 1 ? "1 lap" : `${root.laps.length} laps`
            color: Theme.foreground
            font.pixelSize: Theme.textTitle
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        ListView {
            id: list

            anchors.top: lapsTitle.bottom
            anchors.topMargin: Theme.spaceSmall
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceSmall
            x: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: root.rows

            ScrollFade {
                view: list
                color: Theme.surface
            }

            delegate: Item {
                id: row

                required property var modelData
                readonly property string mark: modelData.split === root.fastest ? "Fastest" : modelData.split === root.slowest ? "Slowest" : ""

                width: list.width
                height: Theme.rowHeight - Theme.spaceMedium

                Text {
                    id: number

                    anchors.verticalCenter: parent.verticalCenter
                    width: 52
                    text: `Lap ${row.modelData.number}`
                    color: Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }

                Text {
                    id: split

                    anchors.left: number.right
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.format(row.modelData.split)
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                    font.features: {
                        "tnum": 1
                    }
                }

                // Beside the lap's own time, which it's about.
                Text {
                    anchors.left: split.right
                    anchors.leftMargin: Theme.spaceSmall
                    anchors.baseline: split.baseline
                    visible: row.mark !== ""
                    text: row.mark
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                Text {
                    id: total

                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.format(row.modelData.total)
                    color: Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                    font.features: {
                        "tnum": 1
                    }
                }
            }
        }

        Text {
            anchors.centerIn: list
            width: list.width
            visible: root.laps.length === 0
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: "Lap notes the time while the stopwatch runs."
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }
}

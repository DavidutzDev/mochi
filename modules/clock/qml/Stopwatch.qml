import QtQuick
import qs.island
import "Time.js" as Time

// The Stopwatch tab: the time big, to the tenth of a second or the
// hundredth as `precision` says, with Start or Pause in the accent color,
// the one accent here, Lap and Reset under it. On the right, the laps,
// newest first, with how long each took and the fastest and slowest named,
// or the past runs, each kept when Reset ends it; Copy hands one, with its
// laps, to the clipboard. The clock module keeps the stopwatch, so it runs
// on with the panel closed; this counts up from when it started.
Item {
    id: root

    property var clock: ({})
    readonly property var watch: clock.stopwatch ?? ({})
    readonly property bool running: watch.since_ms != null
    readonly property var laps: watch.laps ?? []
    readonly property var runs: clock.runs ?? []
    readonly property bool hundredths: clock.precision === "hundredths"
    readonly property bool twelve: clock.hours === "12"
    // "laps" or "runs", on the right.
    property string list: "laps"

    property real now: Date.now()
    readonly property real elapsed: (watch.banked_ms ?? 0) + (running ? Math.max(0, now - watch.since_ms) : 0)

    Timer {
        interval: root.hundredths ? 30 : 50
        repeat: true
        running: root.running && root.visible
        onTriggered: root.now = Date.now()
    }
    onWatchChanged: now = Date.now()

    function pad(value: int): string {
        return `${value}`.padStart(2, "0");
    }

    // m:ss.t, or h:mm:ss.t from an hour, with hundredths as m:ss.cc.
    function format(ms: real): string {
        const fraction = hundredths ? pad(Math.floor(ms / 10) % 100) : `${Math.floor(ms / 100) % 10}`;
        const seconds = Math.floor(ms / 1000);
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor(seconds / 60) % 60;
        const rest = `${pad(seconds % 60)}.${fraction}`;
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
        id: side

        anchors.left: face.right
        anchors.right: parent.right
        height: parent.height
        radius: Theme.radiusSurface
        color: Theme.surface

        Segmented {
            id: pick

            x: Theme.spaceMedium
            y: Theme.spaceMedium
            width: 196
            height: 32
            color: Theme.raised
            options: [
                {
                    "value": "laps",
                    "label": root.laps.length > 0 ? `Laps · ${root.laps.length}` : "Laps"
                },
                {
                    "value": "runs",
                    "label": root.runs.length > 0 ? `Runs · ${root.runs.length}` : "Runs"
                }
            ]
            current: root.list
            keyboard: true
            onPicked: value => root.list = value
        }

        // The run going, or every past one forgotten.
        ActionButton {
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceMedium
            anchors.verticalCenter: pick.verticalCenter
            visible: root.list === "laps" ? root.elapsed > 0 : root.runs.length > 0
            text: root.list === "laps" ? "Copy" : "Clear"
            icon: root.list === "laps" ? "copy" : "trash"
            tone: "ghost"
            onClicked: Daemon.command("clock", root.list === "laps" ? "copy-run" : "forget-run", [])
        }

        ListView {
            id: lapList

            visible: root.list === "laps"
            anchors.top: pick.bottom
            anchors.topMargin: Theme.spaceSmall
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceSmall
            x: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: root.rows

            ScrollFade {
                view: lapList
                color: Theme.surface
            }

            WheelScroll {
                view: lapList
            }

            delegate: Item {
                id: row

                required property var modelData
                readonly property string mark: modelData.split === root.fastest ? "Fastest" : modelData.split === root.slowest ? "Slowest" : ""

                width: lapList.width
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

                // How long it took, next to the lap's name.
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

        ListView {
            id: runList

            visible: root.list === "runs"
            anchors.top: pick.bottom
            anchors.topMargin: Theme.spaceSmall
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceSmall
            x: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: root.runs

            ScrollFade {
                view: runList
                color: Theme.surface
            }

            WheelScroll {
                view: runList
            }

            delegate: Item {
                id: run

                required property var modelData
                required property int index
                readonly property date ended: new Date(modelData.ended * 1000)
                readonly property int lapCount: (modelData.laps ?? []).length

                width: runList.width
                height: Theme.rowHeight - Theme.spaceSmall

                Column {
                    anchors.left: parent.left
                    anchors.right: total.left
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: `${Time.shortDate(run.ended)}, ${Time.time(run.ended, root.twelve)}`
                        color: Theme.foreground
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightLabel
                    }

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: run.lapCount === 0 ? "No laps" : run.lapCount === 1 ? "1 lap" : `${run.lapCount} laps`
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }
                }

                Text {
                    id: total

                    anchors.right: copy.left
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.format(run.modelData.total_ms)
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                    font.features: {
                        "tnum": 1
                    }
                }

                ActionButton {
                    id: copy

                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    icon: "copy"
                    tone: "ghost"
                    onClicked: Daemon.command("clock", "copy-run", [`${run.index + 1}`])
                }
            }
        }

        Text {
            anchors.centerIn: root.list === "laps" ? lapList : runList
            width: lapList.width
            visible: root.list === "laps" ? root.laps.length === 0 : root.runs.length === 0
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: {
                if (root.list === "laps")
                    return "Lap notes the time while the stopwatch runs.";
                if (root.clock.history === 0)
                    return "The history setting keeps no runs.";
                return "Reset keeps the run here, with its laps.";
            }
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }
}

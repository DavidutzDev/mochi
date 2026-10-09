import QtQuick
import qs.island

// The control center's Performance page: CPU, memory and GPU, then disk and
// network, each with its graph of the last two minutes, then the busiest
// processes, in the order the switch picks. Hovering one of the user's own
// processes shows End: a click asks to confirm, the next sends SIGTERM, and
// after 3 seconds one still running offers Force, for SIGKILL.
Item {
    id: root

    property var payload: null
    readonly property var cpu: payload?.cpu ?? null
    readonly property var memory: payload?.memory ?? null
    readonly property var gpu: payload?.gpu ?? null
    readonly property var disk: payload?.disk ?? null
    readonly property var network: payload?.network ?? null
    readonly property var processes: payload?.processes ?? []
    readonly property var critical: payload?.critical ?? []
    readonly property string sort: payload?.sort ?? "cpu"

    // The process whose End button asks to confirm, and when each process
    // was asked to quit, by pid.
    property int confirming: -1
    property var ending: ({})
    property real now: Date.now()

    implicitHeight: column.implicitHeight

    // The process list only updates while someone looks.
    Component.onCompleted: Daemon.command("performance", "detail", [])

    Timer {
        interval: 20000
        running: true
        repeat: true
        onTriggered: Daemon.command("performance", "detail", [])
    }

    // A confirm not taken goes back to End.
    Timer {
        id: confirmTimer

        interval: 4000
        onTriggered: root.confirming = -1
    }

    // Ticks while a process is ending, to offer Force on time.
    Timer {
        interval: 250
        repeat: true
        running: Object.keys(root.ending).length > 0
        onTriggered: root.now = Date.now()
    }

    // Forgets the processes that are gone.
    onProcessesChanged: {
        const kept = {};
        for (const process of processes) {
            if (ending[process.pid] !== undefined)
                kept[process.pid] = ending[process.pid];
        }
        if (Object.keys(kept).length !== Object.keys(ending).length)
            ending = kept;
    }

    function hot(label: string): bool {
        return root.critical.some(entry => entry.label === label);
    }

    function end(pid: int): void {
        Daemon.command("performance", "end", [`${pid}`]);
        root.confirming = -1;
        root.now = Date.now();
        root.ending = Object.assign({}, root.ending, {
            [pid]: root.now
        });
    }

    function force(pid: int): void {
        Daemon.command("performance", "kill", [`${pid}`]);
        // Counting again keeps it on "Ending" until it goes.
        root.now = Date.now();
        root.ending = Object.assign({}, root.ending, {
            [pid]: root.now
        });
    }

    Column {
        id: column

        width: parent.width
        spacing: Theme.spaceSmall

        Row {
            width: parent.width
            spacing: Theme.spaceSmall

            readonly property int count: root.gpu ? 3 : 2
            readonly property real tileWidth: (width - spacing * (count - 1)) / count

            Meter {
                width: parent.tileWidth
                icon: "chip"
                title: "CPU"
                value: `${root.cpu?.usage ?? 0}%`
                detail: [root.cpu?.cores ? `${root.cpu.cores} threads` : "", root.cpu?.temperature != null ? `${root.cpu.temperature} °C` : ""].filter(part => part).join(" · ")
                values: root.cpu?.history ?? []
                hot: root.hot("CPU")
            }

            Meter {
                width: parent.tileWidth
                icon: "memory"
                title: "Memory"
                value: `${root.memory?.percent ?? 0}%`
                detail: root.memory ? `${root.memory.used} of ${root.memory.total}` : ""
                values: root.memory?.history ?? []
                hot: root.hot("Memory")
            }

            Meter {
                visible: root.gpu !== null
                width: parent.tileWidth
                icon: "gpu"
                title: "GPU"
                value: `${root.gpu?.usage ?? 0}%`
                detail: {
                    const parts = [];
                    if (root.gpu?.memory_total)
                        parts.push(`${(root.gpu.memory_used / 1024).toFixed(1)} of ${(root.gpu.memory_total / 1024).toFixed(1)} GB`);
                    if (root.gpu?.temperature != null)
                        parts.push(`${root.gpu.temperature} °C`);
                    return parts.join(" · ");
                }
                values: root.gpu?.history ?? []
                hot: root.hot("GPU")
            }
        }

        Row {
            width: parent.width
            spacing: Theme.spaceSmall

            readonly property real tileWidth: (width - spacing) / 2

            Meter {
                width: parent.tileWidth
                icon: "disk"
                title: "Disk"
                speeds: [["Read", root.disk?.in ?? ""], ["Write", root.disk?.out ?? ""]]
                values: root.disk?.in_history ?? []
                others: root.disk?.out_history ?? []
            }

            Meter {
                width: parent.tileWidth
                icon: "ethernet"
                title: "Network"
                speeds: [["Download", root.network?.in ?? ""], ["Upload", root.network?.out ?? ""]]
                values: root.network?.in_history ?? []
                others: root.network?.out_history ?? []
            }
        }

        Text {
            visible: root.memory?.swap_total && root.memory.swap_total !== "0 MB"
            text: `Swap ${root.memory?.swap_used ?? ""} of ${root.memory?.swap_total ?? ""}${root.gpu ? ` · ${root.gpu.name}` : ""}`
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Item {
            width: parent.width
            height: sorter.height

            SectionLabel {
                anchors.verticalCenter: parent.verticalCenter
                text: "Busiest processes"
            }

            Segmented {
                id: sorter

                anchors.right: parent.right
                width: 240
                height: 32
                options: [
                    {
                        "value": "cpu",
                        "label": "CPU"
                    },
                    {
                        "value": "memory",
                        "label": "Memory"
                    },
                    {
                        "value": "disk",
                        "label": "Disk"
                    }
                ]
                current: root.sort
                onPicked: value => Daemon.command("performance", "sort", [value])
            }
        }

        Text {
            visible: root.processes.length === 0
            text: "Measuring…"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        // A count, not the array: a new array every reading would rebuild
        // the rows.
        Repeater {
            model: root.processes.length

            Item {
                id: process

                required property int index
                readonly property var entry: root.processes[index] ?? {}
                readonly property int pid: entry.pid ?? -1
                readonly property var since: root.ending[pid]
                readonly property string stage: since !== undefined ? (root.now - since >= 3000 ? "force" : "ending") : root.confirming === pid ? "confirm" : "end"

                width: column.width
                height: 30

                HoverHandler {
                    id: hover
                }

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusControl
                    color: process.index % 2 === 0 ? Theme.surface : "transparent"
                }

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: Theme.spaceMedium
                    anchors.right: cpuText.left
                    anchors.rightMargin: Theme.spaceMedium
                    anchors.verticalCenter: parent.verticalCenter
                    text: process.entry.name ?? ""
                    elide: Text.ElideRight
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }

                Cell {
                    id: cpuText

                    anchors.right: memoryText.left
                    width: 60
                    text: `${(process.entry.cpu ?? 0).toFixed(1)}%`
                    sorted: root.sort === "cpu"
                }

                Cell {
                    id: memoryText

                    anchors.right: diskText.left
                    width: 76
                    text: process.entry.memory ?? ""
                    sorted: root.sort === "memory"
                }

                Cell {
                    id: diskText

                    anchors.right: endSlot.left
                    width: 86
                    text: process.entry.disk ?? ""
                    sorted: root.sort === "disk"
                }

                // Room for the button, so the columns don't move.
                Item {
                    id: endSlot

                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceTiny
                    width: 84
                    height: parent.height

                    Button {
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        height: 24
                        visible: process.entry.own === true && (hover.hovered || process.stage !== "end")
                        enabled: process.stage !== "ending"
                        tone: process.stage === "end" ? "neutral" : "danger"
                        text: ({
                                "end": "End",
                                "confirm": "Confirm",
                                "ending": "Ending",
                                "force": "Force"
                            })[process.stage]
                        onClicked: {
                            switch (process.stage) {
                            case "end":
                                root.confirming = process.pid;
                                confirmTimer.restart();
                                break;
                            case "confirm":
                                root.end(process.pid);
                                break;
                            case "force":
                                root.force(process.pid);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // A number in the process list; the column the list is sorted by is
    // brighter.
    component Cell: Text {
        property bool sorted: false

        anchors.verticalCenter: parent.verticalCenter
        horizontalAlignment: Text.AlignRight
        rightPadding: 12
        color: sorted ? Theme.foreground : Theme.muted
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
        font.features: {
            "tnum": 1
        }
    }

    // One reading: its name, value, a detail line and the graph. With
    // `speeds`, two named speeds instead, [[name, speed], [name, speed]],
    // the first drawn in the accent color and the second in green.
    component Meter: Rectangle {
        id: meter

        property string icon: ""
        property string title: ""
        property string value: ""
        property string detail: ""
        property var speeds: []
        property var values: []
        property var others: []
        property bool hot: false

        height: 128
        radius: Theme.radiusSurface
        color: Theme.surface
        border.width: hot ? 1 : 0
        border.color: Theme.danger

        EdgeLight {
            radius: meter.radius
            color: meter.hot ? Theme.danger : Theme.accent
        }

        Row {
            x: 12
            y: 12
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: meter.icon
                size: 13
                color: meter.hot ? Theme.danger : Theme.muted
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: meter.title
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }
        }

        RollingText {
            visible: meter.speeds.length === 0
            x: 12
            y: 32
            text: meter.value
            color: meter.hot ? Theme.danger : Theme.foreground
            pixelSize: Theme.textTitle
            weight: Theme.weightTitle
        }

        Text {
            visible: meter.speeds.length === 0
            x: 12
            y: 60
            width: parent.width - 24
            text: meter.detail
            elide: Text.ElideRight
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Row {
            visible: meter.speeds.length > 0
            x: 12
            y: 32
            spacing: Theme.spaceHuge

            Repeater {
                model: meter.speeds

                Column {
                    required property var modelData
                    required property int index

                    spacing: 2

                    RollingText {
                        text: parent.modelData[1]
                        color: parent.index === 0 ? Theme.accent : Theme.success
                        pixelSize: Theme.textTitle
                        weight: Theme.weightTitle
                    }

                    Text {
                        text: parent.modelData[0]
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }
                }
            }
        }

        Graph {
            x: 1
            y: parent.height - height - 1
            width: parent.width - 2
            height: 40
            values: meter.values
            others: meter.others
            // Speeds scale to the busiest moment, but 64 KB/s at least, so
            // a quiet disk draws a flat line.
            floor: meter.speeds.length > 0 ? 64 * 1024 : 0
            color: meter.hot ? Theme.danger : Theme.accent
        }
    }
}

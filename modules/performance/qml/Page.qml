import QtQuick
import qs.island

// The hub's Performance page: CPU, memory and GPU, each with its graph of
// the last two minutes, then the busiest processes.
Item {
    id: root

    property var payload: null
    readonly property var cpu: payload?.cpu ?? null
    readonly property var memory: payload?.memory ?? null
    readonly property var gpu: payload?.gpu ?? null
    readonly property var processes: payload?.processes ?? []
    readonly property var critical: payload?.critical ?? []

    implicitHeight: column.implicitHeight

    // The process list only updates while someone looks.
    Component.onCompleted: Daemon.command("performance", "detail", [])

    Timer {
        interval: 20000
        running: true
        repeat: true
        onTriggered: Daemon.command("performance", "detail", [])
    }

    function hot(label: string): bool {
        return root.critical.some(entry => entry.label === label);
    }

    Column {
        id: column

        width: parent.width
        spacing: 10

        Row {
            width: parent.width
            spacing: 10

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

        Text {
            visible: root.memory?.swap_total && root.memory.swap_total !== "0 MB"
            text: `Swap ${root.memory?.swap_used ?? ""} of ${root.memory?.swap_total ?? ""}${root.gpu ? ` · ${root.gpu.name}` : ""}`
            color: Theme.muted
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
        }

        SectionLabel {
            topPadding: 4
            text: "Busiest processes"
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

                width: column.width
                height: 30

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusSmall
                    color: process.index % 2 === 0 ? Theme.surface : "transparent"
                }

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: 12
                    anchors.right: cpuText.left
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    text: process.entry.name ?? ""
                    elide: Text.ElideRight
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }

                Text {
                    id: cpuText

                    anchors.right: memoryText.left
                    anchors.rightMargin: 16
                    anchors.verticalCenter: parent.verticalCenter
                    width: 60
                    horizontalAlignment: Text.AlignRight
                    text: `${(process.entry.cpu ?? 0).toFixed(1)}%`
                    color: Theme.muted
                    font.pixelSize: Theme.textLabel
                    font.family: Theme.fontFamily
                    font.features: { "tnum": 1 }
                }

                Text {
                    id: memoryText

                    anchors.right: parent.right
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: 70
                    horizontalAlignment: Text.AlignRight
                    text: process.entry.memory ?? ""
                    color: Theme.muted
                    font.pixelSize: Theme.textLabel
                    font.family: Theme.fontFamily
                    font.features: { "tnum": 1 }
                }
            }
        }
    }

    // One reading: its name, value, a detail line and the graph.
    component Meter: Rectangle {
        id: meter

        property string icon: ""
        property string title: ""
        property string value: ""
        property string detail: ""
        property var values: []
        property bool hot: false

        height: 128
        radius: Theme.radiusLarge
        color: Theme.surface
        border.width: hot ? 1 : 0
        border.color: Theme.danger

        Row {
            x: 12
            y: 12
            spacing: 6

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
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }
        }

        Text {
            x: 12
            y: 32
            text: meter.value
            color: meter.hot ? Theme.danger : Theme.foreground
            font.pixelSize: Theme.textTitle
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
            font.features: { "tnum": 1 }
        }

        Text {
            x: 12
            y: 60
            width: parent.width - 24
            text: meter.detail
            elide: Text.ElideRight
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Graph {
            x: 1
            y: parent.height - height - 1
            width: parent.width - 2
            height: 40
            values: meter.values
            color: meter.hot ? Theme.danger : Theme.accent
        }
    }
}

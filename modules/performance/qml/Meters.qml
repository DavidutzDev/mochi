import QtQuick
import qs.island

// The performance widget's meters look: a bar for the CPU, the memory, the
// GPU when there is one, and the disk the home directory is on, each with
// its percent and what it uses, sharing the widget's height. A reading
// over its critical level turns red.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property var cpu: payload?.cpu ?? null
    readonly property var memory: payload?.memory ?? null
    readonly property var gpu: payload?.gpu ?? null
    readonly property var storage: payload?.storage ?? null
    readonly property var meters: [
        {
            "icon": "chip",
            "label": "CPU",
            "value": cpu?.usage ?? 0,
            "detail": cpu?.temperature != null ? `${cpu.temperature} °C` : ""
        },
        {
            "icon": "memory",
            "label": "Memory",
            "value": memory?.percent ?? 0,
            "detail": memory ? `${memory.used} of ${memory.total}` : ""
        },
        {
            "icon": "gpu",
            "label": "GPU",
            "value": gpu?.usage ?? 0,
            "detail": gpu?.temperature != null ? `${gpu.temperature} °C` : "",
            "missing": gpu === null
        },
        {
            "icon": "disk",
            "label": "Disk",
            "value": storage?.percent ?? 0,
            "detail": storage ? `${storage.used} of ${storage.total}` : "",
            "missing": storage === null
        }
    ].filter(meter => !meter.missing)
    // A meter's line and bar, and as many as fit: a short widget leaves
    // out the last, the disk and then the GPU.
    readonly property real meterHeight: Theme.textBody * 1.3 + Theme.spaceTiny + 6
    readonly property int fit: Math.max(1, Math.floor((height + Theme.spaceSmall) / (meterHeight + Theme.spaceSmall)))

    function hot(label: string): bool {
        return (payload?.critical ?? []).some(entry => entry.label === label);
    }

    Text {
        anchors.centerIn: parent
        visible: root.payload === null
        text: "Waiting for readings"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Column {
        id: list

        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.payload !== null
        spacing: Theme.spaceSmall

        Repeater {
            // By count: a reading changes the rows in place instead of making
            // them again every couple of seconds.
            model: Math.min(root.meters.length, root.fit)

            Column {
                id: meter

                required property int index
                readonly property var entry: root.meters[index] ?? ({})
                readonly property bool hot: root.hot(entry.label ?? "")

                width: list.width
                spacing: Theme.spaceTiny

                Item {
                    width: parent.width
                    height: value.implicitHeight

                    Row {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: Theme.spaceSmall

                        Symbol {
                            anchors.verticalCenter: parent.verticalCenter
                            name: meter.entry.icon ?? ""
                            size: 13
                            color: meter.hot ? Theme.danger : Theme.muted
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: meter.entry.label ?? ""
                            color: Theme.muted
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                        }

                        RollingText {
                            id: value

                            anchors.verticalCenter: parent.verticalCenter
                            text: `${meter.entry.value ?? 0}%`
                            color: meter.hot ? Theme.danger : Theme.foreground
                            pixelSize: Theme.textBody
                            weight: Theme.weightTitle
                        }
                    }

                    Text {
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        text: meter.entry.detail ?? ""
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.features: {
                            "tnum": 1
                        }
                    }
                }

                ProgressBar {
                    width: parent.width
                    height: 6
                    value: (meter.entry.value ?? 0) / 100
                    fill: meter.hot ? Theme.danger : Theme.accent
                }
            }
        }
    }
}

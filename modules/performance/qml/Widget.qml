import QtQuick
import qs.island

// The performance widget: the readings with their last two minutes as
// graphs, stacked, sharing the widget's height. A setting for each graph
// turns it on or off: CPU, memory and GPU show unless set, disk and network
// only when set. A reading over its critical level turns red.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""

    readonly property var cpu: payload?.cpu ?? null
    readonly property var memory: payload?.memory ?? null
    readonly property var gpu: payload?.gpu ?? null
    readonly property var disk: payload?.disk ?? null
    readonly property var network: payload?.network ?? null
    // Before these settings, `reading` picked one graph, or "all".
    readonly property string reading: settings.reading ?? "all"
    readonly property var rows: [
        {
            "key": "cpu",
            "icon": "chip",
            "label": "CPU",
            "value": `${cpu?.usage ?? 0}%`,
            "detail": cpu?.temperature != null ? `${cpu.temperature} °C` : "",
            "values": cpu?.history ?? []
        },
        {
            "key": "memory",
            "icon": "memory",
            "label": "Memory",
            "value": `${memory?.percent ?? 0}%`,
            "detail": memory ? `${memory.used} of ${memory.total}` : "",
            "values": memory?.history ?? []
        },
        {
            "key": "gpu",
            "icon": "gpu",
            "label": "GPU",
            "value": `${gpu?.usage ?? 0}%`,
            "detail": gpu?.temperature != null ? `${gpu.temperature} °C` : "",
            "values": gpu?.history ?? []
        },
        {
            "key": "disk",
            "icon": "disk",
            "label": "Disk",
            "speeds": [["Read", disk?.in ?? ""], ["Write", disk?.out ?? ""]],
            "values": disk?.in_history ?? [],
            "others": disk?.out_history ?? []
        },
        {
            "key": "network",
            "icon": "ethernet",
            "label": "Network",
            "speeds": [["Down", network?.in ?? ""], ["Up", network?.out ?? ""]],
            "values": network?.in_history ?? [],
            "others": network?.out_history ?? []
        }
    ].filter(row => shown(row.key) && (row.key !== "gpu" || gpu !== null))

    function shown(key: string): bool {
        if (reading !== "all")
            return key === reading;
        return settings[key] ?? (key !== "disk" && key !== "network");
    }

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
        id: graphs

        anchors.fill: parent
        spacing: Theme.spaceSmall
        visible: root.payload !== null

        Repeater {
            model: root.rows

            Item {
                id: row

                required property var modelData
                readonly property bool hot: root.hot(modelData.label)
                readonly property var speeds: modelData.speeds ?? []

                width: parent.width
                height: (root.height - graphs.spacing * (root.rows.length - 1)) / root.rows.length

                Row {
                    id: label

                    spacing: Theme.spaceSmall

                    Symbol {
                        anchors.verticalCenter: parent.verticalCenter
                        name: row.modelData.icon
                        size: 13
                        color: row.hot ? Theme.danger : Theme.muted
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.modelData.label
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightTitle
                    }

                    RollingText {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: row.speeds.length === 0
                        text: row.modelData.value ?? ""
                        color: row.hot ? Theme.danger : Theme.foreground
                        pixelSize: Theme.textBody
                        weight: Theme.weightTitle
                    }
                }

                Text {
                    anchors.right: parent.right
                    anchors.verticalCenter: label.verticalCenter
                    visible: row.speeds.length === 0
                    text: row.modelData.detail ?? ""
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                // A disk's or network's two speeds, colored like their lines.
                Row {
                    anchors.right: parent.right
                    anchors.verticalCenter: label.verticalCenter
                    visible: row.speeds.length > 0
                    spacing: Theme.spaceSmall

                    Repeater {
                        model: row.speeds

                        Text {
                            required property var modelData
                            required property int index

                            text: `${modelData[0]} ${modelData[1]}`
                            color: index === 0 ? Theme.accent : Theme.success
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                            font.features: {
                                "tnum": 1
                            }
                        }
                    }
                }

                Graph {
                    anchors.top: label.bottom
                    anchors.topMargin: Theme.spaceTiny
                    anchors.bottom: parent.bottom
                    width: parent.width
                    values: row.modelData.values
                    others: row.modelData.others ?? []
                    floor: row.speeds.length > 0 ? 64 * 1024 : 0
                    color: row.hot ? Theme.danger : Theme.accent
                }
            }
        }
    }
}

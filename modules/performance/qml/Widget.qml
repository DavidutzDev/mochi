import QtQuick
import qs.island

// The performance widget: the readings with their last two minutes as
// graphs, stacked, sharing the widget's height. `reading` picks one, or
// all of them; a reading over its critical level turns red.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""

    readonly property var cpu: payload?.cpu ?? null
    readonly property var memory: payload?.memory ?? null
    readonly property var gpu: payload?.gpu ?? null
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
        }
    ].filter(row => (reading === "all" || row.key === reading) && (row.key !== "gpu" || gpu !== null))

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
        anchors.fill: parent
        spacing: 10
        visible: root.payload !== null

        Repeater {
            model: root.rows

            Item {
                id: row

                required property var modelData
                readonly property bool hot: root.hot(modelData.label)

                width: parent.width
                height: (root.height - 10 * (root.rows.length - 1)) / root.rows.length

                Row {
                    id: label

                    spacing: 6

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
                        font.pixelSize: Theme.textLabel
                        font.family: Theme.fontFamily
                        font.weight: Font.DemiBold
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.modelData.value
                        color: row.hot ? Theme.danger : Theme.foreground
                        font.pixelSize: Theme.textSubtitle
                        font.family: Theme.fontFamily
                        font.weight: Font.DemiBold
                        font.features: { "tnum": 1 }
                    }
                }

                Text {
                    anchors.right: parent.right
                    anchors.verticalCenter: label.verticalCenter
                    text: row.modelData.detail
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                Graph {
                    anchors.top: label.bottom
                    anchors.topMargin: 4
                    anchors.bottom: parent.bottom
                    width: parent.width
                    values: row.modelData.values
                    color: row.hot ? Theme.danger : Theme.accent
                }
            }
        }
    }
}

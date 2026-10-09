import QtQuick
import qs.island

// The performance widget's rings look: CPU, memory and disk, each a ring
// that fills with what's in use, the percent inside and its name under.
// The disk is the one the home directory is on. A reading over its
// critical level turns red. The rings wave unless the settings keep them
// flat.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property var rings: [
        {
            "label": "CPU",
            "value": payload?.cpu?.usage ?? null
        },
        {
            "label": "Memory",
            "value": payload?.memory?.percent ?? null
        },
        {
            "label": "Disk",
            "value": payload?.storage?.percent ?? null
        }
    ]
    // Each ring's column, and the ring as big as the column and the height
    // under its label allow.
    readonly property real column: width / rings.length
    readonly property real size: Math.max(0, Math.min(column - Theme.spaceSmall, height - Theme.textCaption - Theme.spaceSmall * 2))

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

    Row {
        anchors.centerIn: parent
        visible: root.payload !== null

        Repeater {
            model: root.rings

            Column {
                id: ring

                required property var modelData
                readonly property bool known: modelData.value !== null
                readonly property bool hot: root.hot(modelData.label)

                width: root.column
                spacing: Theme.spaceSmall

                WavyRing {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: root.size
                    height: root.size
                    size: root.size
                    thickness: Math.max(3, root.size * 0.07)
                    value: (ring.modelData.value ?? 0) / 100
                    wavy: root.payload?.wavy ?? true
                    color: ring.hot ? Theme.danger : Theme.accent

                    RollingText {
                        anchors.centerIn: parent
                        text: ring.known ? `${ring.modelData.value}%` : "?"
                        color: ring.hot ? Theme.danger : Theme.foreground
                        pixelSize: Math.max(Theme.textCaption, Math.min(Theme.textHeadline, root.size * 0.22))
                        weight: Theme.weightTitle
                    }
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: ring.modelData.label
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                }
            }
        }
    }
}

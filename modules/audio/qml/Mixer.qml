import QtQuick
import qs.island

// The mixer, for the hub page and the island: the output and the input,
// each with a list of devices to switch to, then every app playing sound.
// The width comes from the parent.
Column {
    id: root

    property var payload: null
    // Up to this many apps show; more scroll.
    property int appRows: 4
    readonly property bool connected: payload?.connected ?? false
    readonly property int maxVolume: payload?.max_volume ?? 100
    readonly property var apps: payload?.apps ?? []
    // Which device list is open: "output", "input" or "".
    property string choosing: ""

    spacing: 6

    function level(device: var): string {
        if (device.muted || device.volume === 0)
            return "volume-muted";
        if (device.volume < 34)
            return "volume-1";
        if (device.volume < 67)
            return "volume-2";
        return "volume-3";
    }

    Text {
        width: parent.width
        visible: !root.connected
        topPadding: 12
        bottomPadding: 12
        horizontalAlignment: Text.AlignHCenter
        text: "Can't reach the audio server"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    // Models are counts, not the payload's arrays: a new array would
    // rebuild every row, and lose a slider while it's dragged.
    Repeater {
        model: root.connected ? 2 : 0

        delegate: Column {
            id: section

            required property int index
            readonly property bool output: index === 0
            readonly property var modelData: output
                ? { kind: "output", label: "Output", device: root.payload?.output ?? null, devices: root.payload?.outputs ?? [] }
                : { kind: "input", label: "Input", device: root.payload?.input ?? null, devices: root.payload?.inputs ?? [] }
            readonly property var device: modelData.device

            width: root.width
            spacing: 2

            SectionLabel {
                topPadding: section.index > 0 ? 8 : 0
                text: section.modelData.label
            }

            Text {
                visible: section.device === null
                topPadding: 6
                bottomPadding: 6
                text: section.output ? "No output" : "No microphone"
                color: Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }

            Volume {
                width: parent.width
                visible: section.device !== null
                target: section.modelData.kind
                title: section.device?.description ?? ""
                symbol: section.output ? root.level(section.device ?? {}) : "mic"
                mutedSymbol: section.output ? "volume-muted" : "mic-muted"
                volume: section.device?.volume ?? 0
                muted: section.device?.muted ?? false
                maxVolume: root.maxVolume
                choosable: section.modelData.devices.length > 1
                choosing: root.choosing === section.modelData.kind
                onChoose: root.choosing = choosing ? "" : section.modelData.kind
            }

            Repeater {
                model: root.choosing === section.modelData.kind ? section.modelData.devices.length : 0

                delegate: ListRow {
                    required property int index
                    readonly property var modelData: section.modelData.devices[index] ?? {}

                    width: section.width
                    height: 40
                    flat: true
                    marker: true
                    selected: modelData.default ?? false
                    leadingSize: 22
                    icon: modelData.icon ?? ""
                    title: modelData.description ?? ""
                    onClicked: {
                        Daemon.command("audio", section.modelData.kind, [modelData.name]);
                        root.choosing = "";
                    }
                }
            }
        }
    }

    SectionLabel {
        visible: root.connected
        topPadding: 8
        text: "Apps"
    }

    Text {
        visible: root.connected && root.apps.length === 0
        topPadding: 6
        bottomPadding: 6
        text: "No app is playing sound"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    ListView {
        width: parent.width
        height: Math.min(root.apps.length, root.appRows) * 52
        visible: root.connected && root.apps.length > 0
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: root.apps.length

        delegate: Volume {
            required property int index
            readonly property var modelData: root.apps[index] ?? {}

            width: ListView.view.width
            opacity: (modelData.playing ?? true) ? 1 : 0.6
            target: modelData.id ?? ""
            title: modelData.name ?? ""
            subtitle: modelData.title ?? ""
            appIcon: modelData.icon ?? (modelData.name ?? "").toLowerCase()
            volume: modelData.volume ?? 0
            muted: modelData.muted ?? false
            maxVolume: root.maxVolume
        }
    }
}

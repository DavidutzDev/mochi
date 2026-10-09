import QtQuick
import qs.island

// The mixer, for the control center page and the island: the output and the
// input, each with a list of devices to switch to, then every app playing
// sound, one row per app with its streams inside, and the apps recording
// while any does. Every slider has a peak meter while the mixer shows. The
// width comes from the parent.
Column {
    id: root

    property var payload: null
    // Up to this many apps show; more scroll.
    property int appRows: 4
    readonly property bool connected: payload?.connected ?? false
    readonly property int maxVolume: payload?.max_volume ?? 100
    readonly property var apps: payload?.apps ?? []
    readonly property var recorders: payload?.recorders ?? []
    readonly property var outputs: payload?.outputs ?? []
    // Which device list is open: "output", "input" or "".
    property string choosing: ""
    // The apps showing their streams, by name.
    property var opened: ({})
    // Whose list of outputs to move to is open: app:<name>, stream:<id>
    // or "".
    property string routing: ""
    // The meters' levels, from the module's live values: {output, input,
    // streams, apps}. Null until the first come.
    property var levels: null
    // Names this view to the module, which runs the meters while any view
    // asks for them.
    readonly property string viewer: `mixer-${Math.random().toString(36).slice(2)}`

    Component.onCompleted: Daemon.command("audio", "meters", [viewer, "on"])
    Component.onDestruction: Daemon.command("audio", "meters", [viewer, "off"])

    // Each ask lasts 10 seconds.
    Timer {
        interval: 4000
        running: true
        repeat: true
        onTriggered: Daemon.command("audio", "meters", [root.viewer, "on"])
    }

    Connections {
        target: Daemon

        function onLive(module: string, value: var): void {
            if (module === "audio")
                root.levels = value;
        }
    }

    spacing: Theme.spaceSmall

    function toggleOpen(app: string): void {
        const next = Object.assign({}, opened);
        if (next[app])
            delete next[app];
        else
            next[app] = true;
        opened = next;
    }

    function toggleRouting(key: string): void {
        routing = routing === key ? "" : key;
    }

    // The icon of the output an app plays through, for its button; none
    // when there's nowhere else to move it.
    function routeIcon(output: var): string {
        if (outputs.length < 2)
            return "";
        return outputs.find(device => device.name === output)?.icon ?? "speakers";
    }

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
        topPadding: Theme.spaceMedium
        bottomPadding: Theme.spaceMedium
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
            readonly property var modelData: output ? {
                kind: "output",
                label: "Output",
                device: root.payload?.output ?? null,
                devices: root.payload?.outputs ?? []
            } : {
                kind: "input",
                label: "Input",
                device: root.payload?.input ?? null,
                devices: root.payload?.inputs ?? []
            }
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
                level: root.levels === null ? -1 : (root.levels[section.modelData.kind] ?? 0)
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

    // The outputs to move an app or one of its streams to, under its row.
    component Routes: Column {
        id: routes

        // What `move` names: an app's name or a stream's id.
        property string target: ""
        property string key: ""
        // The output it plays through now.
        property var current: null

        Repeater {
            model: root.routing === routes.key ? root.outputs.length : 0

            delegate: ListRow {
                required property int index
                readonly property var modelData: root.outputs[index] ?? {}

                width: routes.width
                height: 40
                flat: true
                marker: true
                selected: modelData.name === routes.current
                leadingSize: 22
                icon: modelData.icon ?? ""
                title: modelData.description ?? ""
                onClicked: {
                    Daemon.command("audio", "move", [routes.target, modelData.name]);
                    root.routing = "";
                }
            }
        }
    }

    ListView {
        id: appList

        width: parent.width
        height: Math.min(contentHeight, root.appRows * 52)
        visible: root.connected && root.apps.length > 0
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: root.apps.length

        ScrollFade {
            view: appList
        }

        delegate: Column {
            id: group

            required property int index
            readonly property var modelData: root.apps[index] ?? {}
            readonly property string app: modelData.id ?? ""
            readonly property var streams: modelData.streams ?? []
            readonly property bool open: streams.length > 1 && root.opened[app] === true

            width: ListView.view.width

            Volume {
                width: parent.width
                opacity: (group.modelData.playing ?? true) ? 1 : 0.6
                target: group.app
                title: group.modelData.name ?? ""
                subtitle: group.streams.length > 1 ? `${group.streams.length} streams` : (group.modelData.title ?? "")
                appIcon: group.modelData.icon ?? group.app.toLowerCase()
                volume: group.modelData.volume ?? 0
                muted: group.modelData.muted ?? false
                maxVolume: root.maxVolume
                choosable: group.streams.length > 1
                choosing: group.open
                onChoose: root.toggleOpen(group.app)
                routeIcon: root.routeIcon(group.modelData.output)
                routing: root.routing === `app:${group.app}`
                onRoute: root.toggleRouting(`app:${group.app}`)
                level: root.levels === null ? -1 : (root.levels.apps?.[group.app] ?? 0)
            }

            Routes {
                width: parent.width
                target: group.app
                key: `app:${group.app}`
                current: group.modelData.output ?? null
            }

            Repeater {
                model: group.open ? group.streams.length : 0

                delegate: Column {
                    id: single

                    required property int index
                    readonly property var stream: group.streams[index] ?? {}
                    readonly property string streamId: stream.id ?? ""

                    x: 24
                    width: group.width - 24

                    Volume {
                        width: parent.width
                        opacity: (single.stream.playing ?? true) ? 1 : 0.6
                        target: single.streamId
                        title: (single.stream.title ?? "") !== "" ? single.stream.title : (group.modelData.name ?? "")
                        symbol: "music"
                        mutedSymbol: "volume-muted"
                        volume: single.stream.volume ?? 0
                        muted: single.stream.muted ?? false
                        maxVolume: root.maxVolume
                        routeIcon: root.routeIcon(single.stream.output)
                        routing: root.routing === `stream:${single.streamId}`
                        onRoute: root.toggleRouting(`stream:${single.streamId}`)
                        level: root.levels === null ? -1 : (root.levels.streams?.[single.streamId] ?? 0)
                    }

                    Routes {
                        width: parent.width
                        target: single.streamId
                        key: `stream:${single.streamId}`
                        current: single.stream.output ?? null
                    }
                }
            }
        }
    }

    SectionLabel {
        visible: root.connected && root.recorders.length > 0
        topPadding: 8
        text: "Recording"
    }

    // The apps recording, in rows like the apps playing, without outputs to
    // move them to. Their meters show what the input hears at their volume.
    Repeater {
        model: root.connected ? root.recorders.length : 0

        delegate: Column {
            id: recorder

            required property int index
            readonly property var modelData: root.recorders[index] ?? {}
            // What `volume` and `mute` take: recording:<name>.
            readonly property string target: modelData.target ?? ""
            readonly property var streams: modelData.streams ?? []
            readonly property bool open: streams.length > 1 && root.opened[target] === true

            width: root.width

            Volume {
                width: parent.width
                opacity: (recorder.modelData.recording ?? true) ? 1 : 0.6
                target: recorder.target
                title: recorder.modelData.name ?? ""
                subtitle: recorder.streams.length > 1 ? `${recorder.streams.length} streams` : (recorder.modelData.title ?? "")
                appIcon: recorder.modelData.icon ?? (recorder.modelData.name ?? "").toLowerCase()
                symbol: "mic"
                mutedSymbol: "mic-muted"
                volume: recorder.modelData.volume ?? 0
                muted: recorder.modelData.muted ?? false
                maxVolume: root.maxVolume
                choosable: recorder.streams.length > 1
                choosing: recorder.open
                onChoose: root.toggleOpen(recorder.target)
                level: root.levels === null ? -1 : (root.levels.recording?.apps?.[recorder.modelData.id] ?? 0)
            }

            Repeater {
                model: recorder.open ? recorder.streams.length : 0

                delegate: Volume {
                    id: take

                    required property int index
                    readonly property var stream: recorder.streams[index] ?? {}

                    x: 24
                    width: recorder.width - 24
                    opacity: (take.stream.recording ?? true) ? 1 : 0.6
                    target: take.stream.target ?? ""
                    title: (take.stream.title ?? "") !== "" ? take.stream.title : (recorder.modelData.name ?? "")
                    symbol: "mic"
                    mutedSymbol: "mic-muted"
                    volume: take.stream.volume ?? 0
                    muted: take.stream.muted ?? false
                    maxVolume: root.maxVolume
                    level: root.levels === null ? -1 : (root.levels.recording?.streams?.[take.stream.id] ?? 0)
                }
            }
        }
    }
}

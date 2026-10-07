import QtQuick
import qs.island

// The island while picking: the modes, the current one highlighted, and
// what to do next, and for a recording the desktop audio and microphone
// toggles and the quality, whose buttons step through the presets. A click
// on another mode switches to it; the overlay handles the keys (Tab or a
// number switch, A the desktop audio, M the microphone, F the frame rate, Q
// the resolution, Escape cancels).
Item {
    id: root

    property var payload: ({})
    readonly property bool screenshot: payload.kind === "screenshot"
    readonly property bool saving: payload.stage === "saving"

    readonly property var looks: ({
            "region": {
                "label": "Region",
                "icon": "region"
            },
            "window": {
                "label": "Window",
                "icon": "window"
            },
            "screen": {
                "label": "Screen",
                "icon": "display"
            },
            "all": {
                "label": "All screens",
                "icon": "grid"
            }
        })

    readonly property string hint: {
        if (saving)
            return "Saving…";
        const verb = screenshot ? "capture" : "record";
        switch (payload.mode) {
        case "window":
            return `Click a window to ${verb}`;
        case "screen":
            return `Click a screen to ${verb}`;
        case "all":
            return `Click to ${verb} every screen`;
        default:
            return payload.region ? `Enter to ${verb}` : `Drag to ${verb}`;
        }
    }

    implicitWidth: row.implicitWidth + 12
    implicitHeight: row.implicitHeight + 8

    // Sweeps while the capture is being saved.
    EdgeLight {
        radius: root.height / 2
        working: root.saving
    }

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceTiny

        Repeater {
            model: root.payload.modes ?? []

            Button {
                required property string modelData

                anchors.verticalCenter: parent.verticalCenter
                enabled: !root.saving
                icon: root.looks[modelData]?.icon ?? ""
                text: root.looks[modelData]?.label ?? modelData
                tone: root.payload.mode === modelData ? "accent" : "ghost"
                onClicked: {
                    if (root.payload.mode !== modelData)
                        Daemon.command("capture", "mode", [modelData]);
                }
            }
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: 18
            color: Theme.raised
        }

        Item {
            width: Theme.spaceTiny
            height: 1
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.hint
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Item {
            width: Theme.spaceTiny
            height: 1
        }

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.screenshot
            icon: root.payload.audio ? "volume" : "volume-muted"
            tone: root.payload.audio ? "accent" : "ghost"
            size: 15
            onClicked: Daemon.command("capture", "audio", [])
        }

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.screenshot
            icon: root.payload.microphone ? "mic" : "mic-muted"
            tone: root.payload.microphone ? "accent" : "ghost"
            size: 15
            onClicked: Daemon.command("capture", "microphone", [])
        }

        Button {
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.screenshot
            tone: "ghost"
            text: `${root.payload.framerate ?? 60} fps`
            onClicked: Daemon.command("capture", "framerate", [])
        }

        Button {
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.screenshot
            tone: "ghost"
            text: (root.payload.resolution ?? "native") === "native" ? "Native" : root.payload.resolution
            onClicked: Daemon.command("capture", "resolution", [])
        }
    }
}

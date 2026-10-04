import QtQuick
import qs.island

// The island while picking: the modes, the current one highlighted, and
// what to do next. A click on another mode switches to it; the overlay
// handles the keys (Tab or 1 to 3 switch, M the microphone, Escape cancels).
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
        default:
            return payload.region ? `Enter to ${verb}` : `Drag to ${verb}`;
        }
    }

    implicitWidth: row.implicitWidth + 12
    implicitHeight: row.implicitHeight + 8

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 4

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
            width: 4
            height: 1
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.hint
            color: Theme.muted
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
        }

        Item {
            width: 4
            height: 1
        }

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.screenshot
            icon: root.payload.microphone ? "mic" : "mic-muted"
            tone: root.payload.microphone ? "accent" : "ghost"
            size: 15
            onClicked: Daemon.command("capture", "microphone", [])
        }
    }
}

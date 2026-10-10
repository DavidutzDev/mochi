import QtQuick
import qs.island

// The control center's Captures page: the newest screenshots and recordings in
// their folders, whatever made them. A click opens one in the preview card;
// each row can also copy it, edit a screenshot, open its folder or delete it.
// Shift+click or Ctrl+click selects several, and dragging one of them drops
// them all on another app.
Item {
    id: root

    property var payload: null
    readonly property var captures: payload?.captures ?? []
    readonly property bool editable: payload?.editable ?? false
    // Up to five rows show; more scroll.
    readonly property int rows: Math.min(captures.length, 5)
    // The selected captures' paths.
    property var chosen: []
    // A capture deleted or gone leaves the selection.
    onCapturesChanged: chosen = chosen.filter(path => captures.some(capture => capture.path === path))

    implicitHeight: header.height + Theme.spaceMedium + (captures.length === 0 ? 60 : rows * 60 + (rows - 1) * 8)

    // Files other tools saved show up too.
    Component.onCompleted: Daemon.command("capture", "history", [])

    // "now", "5 min ago", "3 h ago", "2 d ago".
    function ago(time: real): string {
        const seconds = Math.max(0, Date.now() / 1000 - time);
        if (seconds < 60)
            return "now";
        if (seconds < 3600)
            return `${Math.floor(seconds / 60)} min ago`;
        if (seconds < 86400)
            return `${Math.floor(seconds / 3600)} h ago`;
        return `${Math.floor(seconds / 86400)} d ago`;
    }

    // The module closes the control center first, so it isn't in the capture.
    function capture(kind: string): void {
        Daemon.command("capture", "start", [kind]);
    }

    // Adds a capture to the selection, or takes it off.
    function choose(path: string): void {
        root.chosen = root.chosen.includes(path) ? root.chosen.filter(other => other !== path) : root.chosen.concat([path]);
    }

    function size(bytes: real): string {
        if (bytes < 1024 * 1024)
            return `${Math.max(1, Math.round(bytes / 1024))} KB`;
        if (bytes < 1024 * 1024 * 1024)
            return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
        return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
    }

    PanelHeader {
        id: header

        width: parent.width
        title: {
            if (root.chosen.length > 0)
                return `${root.chosen.length} selected`;
            const shots = root.captures.filter(capture => capture.kind === "screenshot").length;
            const videos = root.captures.length - shots;
            const plural = (count, word) => `${count} ${word}${count === 1 ? "" : "s"}`;
            return `${plural(shots, "screenshot")} · ${plural(videos, "recording")}`;
        }

        Button {
            visible: root.chosen.length > 0
            icon: "close"
            text: "Clear"
            tone: "ghost"
            onClicked: root.chosen = []
        }

        Button {
            icon: "camera"
            text: "Screenshot"
            onClicked: root.capture("screenshot")
        }

        Button {
            icon: "record"
            text: "Record"
            onClicked: root.capture("record")
        }
    }

    Text {
        anchors.centerIn: list
        visible: root.captures.length === 0
        text: "No captures yet"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    ListView {
        id: list

        anchors.top: header.bottom
        anchors.topMargin: Theme.spaceMedium
        width: parent.width
        height: root.rows * 60 + Math.max(root.rows - 1, 0) * 8
        clip: true
        spacing: Theme.spaceSmall
        boundsBehavior: Flickable.StopAtBounds
        model: root.captures

        ScrollFade {
            view: list
        }

        WheelScroll {
            view: list
        }

        delegate: ListRow {
            id: row

            required property var modelData
            readonly property bool screenshot: modelData.kind === "screenshot"
            // A screenshot shows itself, a recording a frame once it's made.
            readonly property string picturePath: screenshot ? modelData.path : (modelData.thumbnail ?? "")

            width: list.width
            height: 60
            leadingSize: 64
            title: modelData.name ?? ""
            subtitle: [row.screenshot ? "Screenshot" : "Recording", root.ago(modelData.time), root.size(modelData.bytes)].join(" · ")
            onClicked: {
                root.chosen = [];
                Daemon.command("capture", "preview", [row.modelData.path]);
            }
            // Dragged onto an app, it drops the file, or the selection it's in.
            selectable: true
            marker: true
            selected: root.chosen.includes(row.modelData.path)
            files: row.selected ? root.chosen : [row.modelData.path]
            onSelectionToggled: root.choose(row.modelData.path)

            leading: Item {
                anchors.fill: parent

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusControl
                    color: Theme.raised
                    visible: picture.status !== Image.Ready
                }

                Image {
                    id: picture

                    anchors.fill: parent
                    source: row.picturePath ? `file://${row.picturePath.split("/").map(encodeURIComponent).join("/")}` : ""
                    sourceSize.width: 128
                    sourceSize.height: 80
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                }

                // A recording: a camera before its frame comes, then a
                // play mark over it.
                Symbol {
                    anchors.centerIn: parent
                    visible: !row.screenshot
                    name: picture.status === Image.Ready ? "play_circle" : "video"
                    size: 20
                    filled: picture.status === Image.Ready
                    color: picture.status === Image.Ready ? Theme.foreground : Theme.muted
                }
            }

            trailing: [
                IconButton {
                    icon: "copy"
                    size: 14
                    tone: "neutral"
                    onClicked: Daemon.command("capture", "copy", [row.modelData.path])
                },
                IconButton {
                    visible: row.screenshot && root.editable
                    icon: "edit"
                    size: 14
                    tone: "neutral"
                    onClicked: Daemon.command("capture", "edit", [row.modelData.path])
                },
                IconButton {
                    icon: "folder"
                    size: 14
                    tone: "neutral"
                    onClicked: Daemon.command("capture", "open", [row.modelData.path])
                },
                IconButton {
                    icon: "trash"
                    size: 14
                    tone: "neutral"
                    onClicked: Daemon.command("capture", "delete", [row.modelData.path])
                }
            ]
        }
    }
}

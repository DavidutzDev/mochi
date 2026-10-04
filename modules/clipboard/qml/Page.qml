import QtQuick
import qs.island

// The hub page: the history, newest first, with a search box, pause and
// clear above it. Clicking a row pastes it into the window you were in, the
// copy button only copies it, the trash removes it.
Item {
    id: root

    property var payload: null
    readonly property var entries: payload?.entries ?? []
    readonly property bool paused: payload?.paused ?? false
    // Filtered here: the page lists the newest entries the module publishes.
    readonly property var shown: {
        const query = search.text.trim().toLowerCase();
        if (query === "")
            return entries;
        return entries.filter(entry => (entry.kind === "image" ? "image" : entry.text).toLowerCase().includes(query));
    }
    // Up to five rows show; more scroll.
    readonly property int rows: Math.min(shown.length, 5)

    implicitHeight: toolbar.height + 12 + (shown.length === 0 ? 60 : rows * 60 + (rows - 1) * 8)

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

    // The first line with something on it, spaces squeezed.
    function headline(text: string): string {
        const line = text.split("\n").find(line => line.trim() !== "") ?? "";
        return line.trim().replace(/\s+/g, " ");
    }

    Item {
        id: toolbar

        width: parent.width
        height: 32

        Rectangle {
            anchors.left: parent.left
            anchors.right: buttons.left
            anchors.rightMargin: 12
            height: parent.height
            radius: height / 2
            color: Theme.surface

            Symbol {
                id: magnifier

                x: 12
                anchors.verticalCenter: parent.verticalCenter
                name: "search"
                size: 14
                color: Theme.muted
            }

            TextInput {
                id: search

                anchors.left: magnifier.right
                anchors.leftMargin: 8
                anchors.right: parent.right
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                clip: true

                Text {
                    visible: search.text === ""
                    text: root.paused ? "Paused" : root.entries.length === 1 ? "1 entry" : `${root.entries.length} entries`
                    color: Theme.muted
                    font: search.font
                }
            }
        }

        Row {
            id: buttons

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            Button {
                text: root.paused ? "Resume" : "Pause"
                onClicked: Daemon.command("clipboard", "pause", ["toggle"])
            }

            Button {
                visible: root.entries.length > 0
                text: "Clear all"
                onClicked: Daemon.command("clipboard", "clear", [])
            }
        }
    }

    Text {
        anchors.centerIn: list
        visible: root.shown.length === 0
        text: search.text === "" ? "Nothing copied yet" : "No matches"
        color: Theme.muted
        font.pixelSize: Theme.textSubtitle
        font.family: Theme.fontFamily
    }

    ListView {
        id: list

        anchors.top: toolbar.bottom
        anchors.topMargin: 12
        width: parent.width
        height: root.rows * 60 + Math.max(root.rows - 1, 0) * 8
        clip: true
        spacing: 8
        boundsBehavior: Flickable.StopAtBounds
        model: root.shown

        delegate: ListRow {
            id: row

            required property var modelData
            readonly property bool image: modelData.kind === "image"

            width: list.width
            height: 60
            leadingSize: 40
            title: image ? "Image" : root.headline(modelData.text)
            subtitle: {
                const parts = [];
                if (image && modelData.width > 0)
                    parts.push(`${modelData.width} × ${modelData.height}`);
                if (!image && modelData.lines > 1)
                    parts.push(`${modelData.lines} lines`);
                parts.push(root.ago(modelData.time));
                return parts.join(" · ");
            }
            onClicked: Daemon.command("clipboard", "pick", [`${row.modelData.id}`])

            leading: Item {
                anchors.fill: parent

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusSmall
                    color: Theme.raised
                    visible: !row.image || picture.status !== Image.Ready
                }

                Symbol {
                    anchors.centerIn: parent
                    visible: !row.image
                    name: "clipboard"
                    size: 20
                    color: Theme.muted
                }

                Image {
                    id: picture

                    anchors.fill: parent
                    visible: row.image
                    source: row.modelData.image ?? ""
                    sourceSize.width: 80
                    sourceSize.height: 80
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    cache: false
                }
            }

            trailing: [
                IconButton {
                    icon: "copy"
                    size: 14
                    tone: "neutral"
                    onClicked: Daemon.command("clipboard", "copy", [`${row.modelData.id}`])
                },
                IconButton {
                    icon: "trash"
                    size: 14
                    tone: "neutral"
                    onClicked: Daemon.command("clipboard", "delete", [`${row.modelData.id}`])
                }
            ]
        }
    }
}

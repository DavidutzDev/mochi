import QtQuick
import qs.island

// The control center page: the pins, then the history, newest first, with a
// search box, pause and clear above them. Clicking a row pastes it into the
// window you were in, the pin button pins or unpins it, the copy button only
// copies it, the trash removes it. Clearing leaves the pins.
Item {
    id: root

    property var payload: null
    readonly property var entries: payload?.entries ?? []
    readonly property int count: payload?.count ?? 0
    readonly property bool paused: payload?.paused ?? false
    // Filtered here: the page lists the newest entries the module publishes.
    readonly property var shown: {
        const query = search.text.trim().toLowerCase();
        if (query === "")
            return entries;
        return entries.filter(entry => (entry.kind === "image" ? "image" : entry.text).toLowerCase().includes(query));
    }
    readonly property bool hasPins: shown.length > 0 && shown[0].pinned
    readonly property int headingHeight: 24
    // Up to five rows show; more scroll.
    readonly property int rows: Math.min(shown.length, 5)

    implicitHeight: toolbar.height + Theme.spaceMedium + (shown.length === 0 ? 60 : rows * 60 + (rows - 1) * 8 + (hasPins ? 2 * headingHeight : 0))

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

    // "Pinned" over the first pin, "History" over the first entry after
    // them, or nothing.
    function heading(index: int): string {
        const entry = shown[index];
        if (!entry || !hasPins)
            return "";
        if (index === 0)
            return "Pinned";
        if (!entry.pinned && shown[index - 1].pinned)
            return "History";
        return "";
    }

    Item {
        id: toolbar

        width: parent.width
        height: 32

        Rectangle {
            anchors.left: parent.left
            anchors.right: buttons.left
            anchors.rightMargin: Theme.spaceMedium
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
                anchors.leftMargin: Theme.spaceSmall
                anchors.right: parent.right
                anchors.rightMargin: Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                clip: true

                Text {
                    visible: search.text === ""
                    text: root.paused ? "Paused" : root.count === 1 ? "1 entry" : `${root.count} entries`
                    color: Theme.muted
                    font: search.font
                }
            }
        }

        Row {
            id: buttons

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceSmall

            Button {
                text: root.paused ? "Resume" : "Pause"
                onClicked: Daemon.command("clipboard", "pause", ["toggle"])
            }

            Button {
                visible: root.count > 0
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
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    ListView {
        id: list

        anchors.top: toolbar.bottom
        anchors.topMargin: Theme.spaceMedium
        width: parent.width
        height: root.rows * 60 + Math.max(root.rows - 1, 0) * 8 + (root.hasPins ? 2 * root.headingHeight : 0)
        clip: true
        spacing: Theme.spaceSmall
        boundsBehavior: Flickable.StopAtBounds
        model: root.shown

        ScrollFade {
            view: list
        }

        delegate: Column {
            id: item

            required property var modelData
            required property int index
            readonly property string heading: root.heading(index)

            width: list.width

            Text {
                visible: item.heading !== ""
                width: parent.width
                height: root.headingHeight
                leftPadding: 4
                verticalAlignment: Text.AlignVCenter
                text: item.heading
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            ListRow {
                id: row

                readonly property var modelData: item.modelData
                readonly property bool image: modelData.kind === "image"
                readonly property bool pinned: modelData.pinned ?? false
                readonly property string entry: `${modelData.id}`

                width: parent.width
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
                // An image opens in the preview card, with copy, edit and delete.
                onClicked: Daemon.command("clipboard", row.image ? "show" : "pick", [row.entry])
                // An image dragged onto an app drops it as a file.
                file: row.image ? (row.modelData.image ?? "").replace(/^file:\/\//, "") : ""

                // Over the buttons too, which the row's own hover misses.
                HoverHandler {
                    id: hover
                }

                leading: Item {
                    anchors.fill: parent

                    Rectangle {
                        anchors.fill: parent
                        radius: Theme.radiusControl
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
                        // Always on a pin; on hover elsewhere.
                        opacity: row.pinned || hover.hovered ? 1 : 0
                        enabled: opacity > 0
                        icon: "pin"
                        size: 14
                        tone: row.pinned ? "accent" : "neutral"
                        onClicked: Daemon.command("clipboard", row.pinned ? "unpin" : "pin", [row.entry])
                    },
                    IconButton {
                        icon: "copy"
                        size: 14
                        tone: "neutral"
                        onClicked: Daemon.command("clipboard", "copy", [row.entry])
                    },
                    IconButton {
                        icon: "trash"
                        size: 14
                        tone: "neutral"
                        onClicked: Daemon.command("clipboard", "delete", [row.entry])
                    }
                ]
            }
        }
    }
}

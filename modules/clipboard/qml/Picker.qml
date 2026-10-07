import QtQuick
import qs.island

// The history: a search box, the entries with the pins on top, and the
// selected one in full on the right. Each keystroke goes to the module,
// which answers with the matches. Arrows or Tab move the selection, Enter
// pastes it, Shift+Enter only copies it, Ctrl+P pins or unpins it,
// Shift+Delete removes it, Page Up and Page Down scroll the side pane,
// Escape closes. A click on an image opens it in the preview card a
// screenshot gets.
Item {
    id: root

    property var payload: ({})
    // Results only count for what is typed now; older answers still on the
    // way are ignored.
    property var results: []
    readonly property int rows: 7
    readonly property int rowHeight: 56
    readonly property int headingHeight: 26
    readonly property var current: results[list.currentIndex] ?? null
    // Each answer is a new list, so changes of `current` aren't all moves.
    readonly property real currentId: current?.id ?? -1
    readonly property bool hasPins: results.length > 0 && results[0].pinned
    // The whole text of the selected entry, once the module sent it.
    readonly property string detail: {
        const detail = payload.detail;
        if (current && detail && detail.id === current.id)
            return detail.text;
        return current?.text ?? "";
    }

    implicitWidth: 880
    implicitHeight: column.implicitHeight + Theme.spaceSmall * 2

    onPayloadChanged: {
        if ((payload.query ?? "") === input.text) {
            const selected = results[list.currentIndex]?.id;
            results = payload.results ?? [];
            // Keep the selection on the same entry when the list changes
            // under it, as after a removal or a pin.
            const index = results.findIndex(entry => entry.id === selected);
            list.currentIndex = index >= 0 ? index : Math.min(list.currentIndex, results.length - 1);
            if (list.currentIndex < 0)
                list.currentIndex = 0;
        }
    }

    // The side pane shows a text in full: ask for it whenever the
    // selection lands on one.
    onCurrentIdChanged: {
        pane.contentY = 0;
        if (current && current.kind === "text" && payload.detail?.id !== current.id)
            Daemon.command("clipboard", "preview", [`${current.id}`]);
    }

    Component.onCompleted: {
        results = payload.results ?? [];
        Qt.callLater(() => input.forceActiveFocus());
    }

    function send(action: string, index: int): void {
        const entry = results[index];
        if (!entry)
            return;
        Daemon.command("clipboard", action, [`${entry.id}`]);
        // Copied or pinned without closing: the panel's edge says it's done.
        if (action === "copy" || action === "pin")
            light.flash();
    }

    function togglePin(index: int): void {
        const entry = results[index];
        if (entry)
            send(entry.pinned ? "unpin" : "pin", index);
    }

    function move(by: int): void {
        if (results.length > 0)
            list.currentIndex = (list.currentIndex + by + results.length) % results.length;
    }

    function scrollPane(by: real): void {
        const most = Math.max(0, pane.contentHeight - pane.height);
        pane.contentY = Math.max(0, Math.min(most, pane.contentY + by));
    }

    // "now", "5 min", "3 h", "2 d", from seconds since the epoch.
    function ago(time: real): string {
        const seconds = Math.max(0, (payload.now ?? time) - time);
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

    // The heading above a row: "Pinned" over the first pin, "History"
    // over the first entry after them, or nothing.
    function heading(index: int): string {
        const entry = results[index];
        if (!entry || !hasPins)
            return "";
        if (index === 0)
            return "Pinned";
        if (!entry.pinned && results[index - 1].pinned)
            return "History";
        return "";
    }

    EdgeLight {
        id: light

        radius: Theme.radiusSurface
    }

    Column {
        id: column

        anchors.fill: parent
        anchors.topMargin: Theme.spaceSmall
        anchors.bottomMargin: Theme.spaceSmall

        Item {
            width: parent.width
            height: 48

            Symbol {
                id: magnifier

                x: Theme.padding + 4
                anchors.verticalCenter: parent.verticalCenter
                name: "search"
                size: 18
                color: Theme.muted
            }

            TextInput {
                id: input

                anchors.left: magnifier.right
                anchors.leftMargin: Theme.spaceMedium
                anchors.right: pausedLabel.left
                anchors.rightMargin: Theme.padding
                anchors.verticalCenter: parent.verticalCenter
                focus: true
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                clip: true

                onTextChanged: Daemon.command("clipboard", "search", text ? [text] : [])

                Keys.onUpPressed: root.move(-1)
                Keys.onDownPressed: root.move(1)
                Keys.onTabPressed: root.move(1)
                Keys.onBacktabPressed: root.move(-1)
                Keys.onReturnPressed: event => root.send(event.modifiers & Qt.ShiftModifier ? "copy" : "pick", list.currentIndex)
                Keys.onEnterPressed: event => root.send(event.modifiers & Qt.ShiftModifier ? "copy" : "pick", list.currentIndex)
                Keys.onEscapePressed: Daemon.event("dismiss")
                Keys.onDeletePressed: event => {
                    if (event.modifiers & Qt.ShiftModifier)
                        root.send("delete", list.currentIndex);
                    else
                        event.accepted = false;
                }
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_P && event.modifiers & Qt.ControlModifier) {
                        root.togglePin(list.currentIndex);
                        event.accepted = true;
                    } else if (event.key === Qt.Key_PageDown) {
                        root.scrollPane(pane.height - 40);
                        event.accepted = true;
                    } else if (event.key === Qt.Key_PageUp) {
                        root.scrollPane(-(pane.height - 40));
                        event.accepted = true;
                    }
                }

                Text {
                    visible: input.text === ""
                    text: "Search the clipboard…"
                    color: Theme.muted
                    font: input.font
                }
            }

            Text {
                id: pausedLabel

                anchors.right: parent.right
                anchors.rightMargin: Theme.padding + 4
                anchors.verticalCenter: parent.verticalCenter
                visible: root.payload.paused ?? false
                width: visible ? implicitWidth : 0
                text: "Paused"
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        Rectangle {
            width: parent.width
            height: 1
            color: Theme.raised
        }

        Text {
            visible: root.results.length === 0
            width: parent.width
            height: root.rowHeight
            leftPadding: Theme.padding + 4
            verticalAlignment: Text.AlignVCenter
            text: input.text === "" ? "Nothing copied yet" : "No matches"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Item {
            visible: root.results.length > 0
            width: parent.width
            height: Math.min(root.results.length, root.rows) * root.rowHeight + (root.hasPins ? 2 * root.headingHeight : 0) + 8

            ListView {
                id: list

                width: 440
                height: parent.height
                topMargin: Theme.spaceSmall
                clip: true
                model: root.results
                boundsBehavior: Flickable.StopAtBounds
                highlightMoveDuration: 0

                ScrollFade {
                    view: list
                }

                delegate: Column {
                    id: item

                    required property var modelData
                    required property int index
                    readonly property string heading: root.heading(index)

                    x: Theme.spaceSmall
                    width: list.width - Theme.spaceSmall * 2

                    Text {
                        visible: item.heading !== ""
                        width: parent.width
                        height: root.headingHeight
                        leftPadding: 14
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
                        readonly property int index: item.index
                        readonly property bool image: modelData.kind === "image"

                        width: parent.width
                        height: root.rowHeight
                        flat: true
                        marker: true
                        selected: item.ListView.isCurrentItem
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
                        onHoveredChanged: {
                            if (hovered)
                                list.currentIndex = index;
                        }
                        // An image opens in the preview card; Enter still pastes it.
                        onClicked: root.send(image ? "show" : "pick", index)

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
                                visible: row.selected || row.modelData.pinned
                                icon: "pin"
                                size: 14
                                tone: row.modelData.pinned ? "accent" : "ghost"
                                onClicked: root.togglePin(row.index)
                            },
                            IconButton {
                                visible: row.selected
                                icon: "trash"
                                size: 14
                                onClicked: root.send("delete", row.index)
                            }
                        ]
                    }
                }
            }

            // The selected entry in full: the whole text, scrolling, or
            // the image at a size that fits.
            Rectangle {
                anchors.left: list.right
                anchors.right: parent.right
                anchors.rightMargin: Theme.spaceSmall
                anchors.top: parent.top
                anchors.topMargin: Theme.spaceSmall
                anchors.bottom: parent.bottom
                radius: Theme.radiusField
                color: Theme.surface
                clip: true

                Flickable {
                    id: pane

                    anchors.fill: parent
                    anchors.margins: Theme.spaceMedium
                    visible: root.current?.kind === "text"
                    contentWidth: width
                    contentHeight: full.implicitHeight
                    boundsBehavior: Flickable.StopAtBounds
                    clip: true

                    ScrollFade {
                        view: pane
                        color: Theme.surface
                        size: Theme.spaceMedium
                    }

                    Text {
                        id: full

                        width: pane.width
                        text: root.detail
                        wrapMode: Text.WrapAnywhere
                        textFormat: Text.PlainText
                        color: Theme.foreground
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }
                }

                Image {
                    anchors.fill: parent
                    anchors.margins: Theme.spaceMedium
                    visible: root.current?.kind === "image"
                    source: root.current?.kind === "image" ? (root.current.image ?? "") : ""
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    cache: false
                }
            }
        }

        Text {
            width: parent.width
            topPadding: 6
            leftPadding: Theme.padding + 4
            visible: root.results.length > 0
            text: "Enter pastes · Shift+Enter copies · Ctrl+P pins · Shift+Delete removes"
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }
}

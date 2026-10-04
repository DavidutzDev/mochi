import QtQuick
import qs.island

// The history: a search box and the entries, newest first. Each keystroke
// goes to the module, which answers with the matches. Arrows or Tab move
// the selection, Enter pastes it, Shift+Enter only copies it, Shift+Delete
// removes it, Escape closes. A click on an image opens it in the preview
// card a screenshot gets.
Item {
    id: root

    property var payload: ({})
    // Results only count for what is typed now; older answers still on the
    // way are ignored.
    property var results: []
    readonly property int rows: 7
    readonly property int rowHeight: 56

    implicitWidth: 600
    implicitHeight: column.implicitHeight + 16

    onPayloadChanged: {
        if ((payload.query ?? "") === input.text) {
            const selected = results[list.currentIndex]?.id;
            results = payload.results ?? [];
            // Keep the selection on the same entry when the list changes
            // under it, as after a removal.
            const index = results.findIndex(entry => entry.id === selected);
            list.currentIndex = index >= 0 ? index : Math.min(list.currentIndex, results.length - 1);
            if (list.currentIndex < 0)
                list.currentIndex = 0;
        }
    }

    Component.onCompleted: {
        results = payload.results ?? [];
        Qt.callLater(() => input.forceActiveFocus());
    }

    function send(action: string, index: int): void {
        const entry = results[index];
        if (entry)
            Daemon.command("clipboard", action, [`${entry.id}`]);
    }

    function move(by: int): void {
        if (results.length > 0)
            list.currentIndex = (list.currentIndex + by + results.length) % results.length;
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

    Column {
        id: column

        anchors.fill: parent
        anchors.topMargin: 8
        anchors.bottomMargin: 8

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
                anchors.leftMargin: 12
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
                font.pixelSize: Theme.textLabel
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

        ListView {
            id: list

            visible: root.results.length > 0
            width: parent.width
            height: Math.min(root.results.length, root.rows) * root.rowHeight + 8
            topMargin: 8
            clip: true
            model: root.results
            boundsBehavior: Flickable.StopAtBounds
            delegate: ListRow {
                id: row

                required property var modelData
                required property int index
                readonly property bool image: modelData.kind === "image"

                x: 8
                width: list.width - 16
                height: root.rowHeight
                flat: true
                marker: true
                selected: ListView.isCurrentItem
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

                trailing: IconButton {
                    visible: row.selected
                    icon: "trash"
                    size: 16
                    onClicked: root.send("delete", row.index)
                }
            }
        }

        Text {
            width: parent.width
            topPadding: 6
            leftPadding: Theme.padding + 4
            visible: root.results.length > 0
            text: "Enter pastes · Shift+Enter copies · Shift+Delete removes"
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }
}

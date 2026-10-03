import QtQuick
import Quickshell
import qs.island

// The search box and the results. Each keystroke goes to the module, which
// answers with new results for that query. The selection lives here: arrows
// or Tab move it, Enter starts it, Escape closes, the pointer selects on
// hover and starts on click.
Item {
    id: root

    property var payload: ({})
    // Results only count for what is typed now; older answers still on the
    // way are ignored.
    property var results: []
    readonly property int rows: 7
    readonly property int rowHeight: 50

    implicitWidth: 560
    implicitHeight: column.implicitHeight + 16

    onPayloadChanged: {
        if ((payload.query ?? "") === input.text) {
            results = payload.results ?? [];
            list.currentIndex = 0;
        }
    }

    Component.onCompleted: {
        results = payload.results ?? [];
        Qt.callLater(() => input.forceActiveFocus());
    }

    function launch(index: int): void {
        const result = results[index];
        if (result)
            Daemon.command("launcher", "launch", [result.id]);
    }

    function move(by: int): void {
        if (results.length > 0)
            list.currentIndex = (list.currentIndex + by + results.length) % results.length;
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
                anchors.right: parent.right
                anchors.rightMargin: Theme.padding
                anchors.verticalCenter: parent.verticalCenter
                focus: true
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                clip: true

                onTextChanged: Daemon.command("launcher", "search", text ? [text] : [])

                Keys.onUpPressed: root.move(-1)
                Keys.onDownPressed: root.move(1)
                Keys.onTabPressed: root.move(1)
                Keys.onBacktabPressed: root.move(-1)
                Keys.onReturnPressed: root.launch(list.currentIndex)
                Keys.onEnterPressed: root.launch(list.currentIndex)
                Keys.onEscapePressed: Daemon.event("dismiss")

                Text {
                    visible: input.text === ""
                    text: "Search…"
                    color: Theme.muted
                    font: input.font
                }
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
            text: "No apps found"
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

                x: 8
                width: list.width - 16
                height: root.rowHeight
                flat: true
                marker: true
                selected: ListView.isCurrentItem
                leadingSize: 32
                title: modelData.name
                subtitle: modelData.description ?? ""
                onHoveredChanged: {
                    if (hovered)
                        list.currentIndex = index;
                }
                onClicked: root.launch(index)

                leading: Item {
                    anchors.fill: parent

                    Image {
                        id: icon

                        anchors.fill: parent
                        // Actions get a smaller icon, a step in.
                        anchors.margins: row.modelData.action ? 6 : 0
                        source: {
                            const name = row.modelData.icon ?? "";
                            if (name.startsWith("/"))
                                return `file://${name}`;
                            return name ? Quickshell.iconPath(name, true) : "";
                        }
                        sourceSize.width: 64
                        sourceSize.height: 64
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                    }

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 4
                        visible: icon.status !== Image.Ready
                        radius: Theme.radiusSmall
                        color: Theme.raised
                    }
                }
            }
        }
    }
}

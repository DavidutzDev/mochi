import QtQuick
import QtQuick.Shapes
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

            Shape {
                id: magnifier

                x: Theme.padding + 4
                anchors.verticalCenter: parent.verticalCenter
                width: 18
                height: 18
                preferredRendererType: Shape.CurveRenderer

                ShapePath {
                    fillColor: Theme.muted
                    strokeColor: "transparent"
                    fillRule: ShapePath.OddEvenFill
                    scale: Qt.size(18 / 24, 18 / 24)
                    PathSvg {
                        path: "M10 3a7 7 0 1 0 4.2 12.6l4.6 4.6 1.4-1.4-4.6-4.6A7 7 0 0 0 10 3zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10z"
                    }
                }
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
                font.pixelSize: 16
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
            color: Theme.surface
        }

        Text {
            visible: root.results.length === 0
            width: parent.width
            height: root.rowHeight
            leftPadding: Theme.padding + 4
            verticalAlignment: Text.AlignVCenter
            text: "No apps found"
            color: Theme.muted
            font.pixelSize: 13
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
            highlightMoveDuration: 120

            highlight: Rectangle {
                x: 8
                width: list.width - 16
                radius: 12
                color: Theme.surface

                Rectangle {
                    x: 0
                    anchors.verticalCenter: parent.verticalCenter
                    width: 3
                    height: parent.height - 18
                    radius: 1.5
                    color: Theme.accent
                }
            }

            delegate: Item {
                id: row

                required property var modelData
                required property int index

                width: list.width
                height: root.rowHeight

                Row {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.padding + 4
                    anchors.rightMargin: Theme.padding
                    spacing: 12

                    Item {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 32
                        height: 32

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
                            radius: 8
                            color: Theme.surface
                        }
                    }

                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - 44

                        Text {
                            width: parent.width
                            text: row.modelData.name
                            elide: Text.ElideRight
                            color: Theme.foreground
                            font.pixelSize: 14
                            font.weight: Font.DemiBold
                        }

                        Text {
                            width: parent.width
                            visible: text !== ""
                            text: row.modelData.description ?? ""
                            elide: Text.ElideRight
                            color: Theme.muted
                            font.pixelSize: 12
                        }
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onEntered: list.currentIndex = row.index
                    onClicked: root.launch(row.index)
                }
            }
        }
    }
}

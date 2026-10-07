import QtQuick
import qs.island
import "Place.js" as Place

// While arranging, under the island: every widget the running modules
// offer, with a search and a filter by module, and the buttons to copy the
// layout and to stop. A click on the island's notice opens it; it folds
// once the pointer leaves it, and while a widget is dragged out of it.
Item {
    id: root

    required property Item desktop

    anchors.fill: parent

    // What's being dragged out, or null: an entry of the catalog.
    property var dragging: null
    property point at
    property string query: ""
    // The module the list is narrowed to, or "" for all.
    property string module: ""
    property bool menu: false

    readonly property bool atBottom: Theme.anchor === "bottom"
    readonly property bool open: (desktop.layout?.drawer ?? false) && dragging === null
    readonly property var modules: [...new Set(desktop.catalog.map(entry => entry.module))].sort()
    readonly property var shown: desktop.catalog.filter(entry => {
        if (module !== "" && entry.module !== module)
            return false;
        const words = query.toLowerCase().split(/\s+/).filter(word => word !== "");
        const text = `${entry.title} ${entry.module} ${entry.widget}`.toLowerCase();
        return words.every(word => text.includes(word));
    })

    function close(): void {
        menu = false;
        Daemon.command("widgets", "drawer", ["off"]);
    }

    onOpenChanged: {
        if (open)
            unvisited.restart();
    }

    // Opened but never pointed at: it goes after a while.
    Timer {
        id: unvisited

        interval: 2500
        onTriggered: {
            if (!hover.hovered)
                root.close();
        }
    }

    // Left: it folds a moment later, so crossing a gap doesn't close it.
    Timer {
        id: leaving

        interval: 400
        onTriggered: {
            if (!hover.hovered && !root.menu)
                root.close();
        }
    }

    Rectangle {
        id: pill

        readonly property real openHeight: Math.min(root.height - 140, 560)
        // Under the island and its bubbles.
        readonly property real gap: Theme.margin + Theme.idleHeight + 10

        anchors.horizontalCenter: parent.horizontalCenter
        y: root.atBottom ? root.height - height - gap : gap
        width: 400
        height: root.open ? openHeight : 0
        visible: height > 1 || root.dragging !== null
        radius: Math.min(height / 2, Theme.radiusSurface)
        color: Theme.background
        border.width: 1
        border.color: Theme.border
        clip: true

        Behavior on height {
            SpringAnimation {
                spring: Theme.spring
                damping: Theme.damping
                epsilon: 0.25
            }
        }

        HoverHandler {
            id: hover

            onHoveredChanged: {
                if (!hovered)
                    leaving.restart();
            }
        }

        // Clicks here stay here.
        MouseArea {
            anchors.fill: parent
        }

        // Flashes when the layout is copied.
        EdgeLight {
            id: light

            radius: pill.radius
        }

        // Open: the drawer. It stays while a widget is dragged out of it,
        // only faded: the entry being dragged holds the pointer.
        Item {
            anchors.fill: parent
            anchors.margins: Theme.padding
            opacity: root.open ? 1 : 0
            visible: opacity > 0 || root.dragging !== null

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.fast
                }
            }

            Column {
                id: header

                width: parent.width
                spacing: Theme.spaceSmall

                PanelHeader {
                    width: parent.width
                    title: "Widgets"
                }

                Rectangle {
                    width: parent.width
                    height: 34
                    radius: height / 2
                    color: Theme.raised
                    border.width: search.activeFocus ? 1 : 0
                    border.color: Theme.accent

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
                        onTextChanged: root.query = text
                        Keys.onEscapePressed: {
                            if (text !== "")
                                text = "";
                            else
                                focus = false;
                        }

                        Text {
                            visible: search.text === ""
                            text: "Search widgets"
                            color: Theme.muted
                            font: search.font
                        }
                    }
                }

                // One chip per module offering widgets.
                Flow {
                    width: parent.width
                    spacing: Theme.spaceSmall

                    Repeater {
                        model: [""].concat(root.modules)

                        Rectangle {
                            id: chip

                            required property string modelData
                            readonly property bool chosen: root.module === modelData

                            width: label.implicitWidth + 20
                            height: 26
                            radius: height / 2
                            color: chosen ? Theme.foreground : chipArea.containsMouse ? Theme.highlight : Theme.raised

                            Text {
                                id: label

                                anchors.centerIn: parent
                                text: chip.modelData === "" ? "All" : chip.modelData
                                color: chip.chosen ? Theme.background : Theme.foreground
                                font.pixelSize: Theme.textCaption
                                font.family: Theme.fontFamily
                                font.weight: Theme.weightTitle
                            }

                            MouseArea {
                                id: chipArea

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.module = chip.modelData
                            }
                        }
                    }
                }
            }

            ListView {
                id: list

                anchors.top: header.bottom
                anchors.topMargin: Theme.spaceSmall
                anchors.bottom: buttons.top
                anchors.bottomMargin: Theme.spaceSmall
                width: parent.width
                clip: true
                spacing: Theme.spaceTiny
                model: root.shown
                boundsBehavior: Flickable.StopAtBounds
                // Keeps every entry while the drawer folds under a drag.
                cacheBuffer: 4000

                ScrollFade {
                    view: list
                }

                delegate: ListRow {
                    id: entry

                    required property var modelData

                    width: list.width
                    title: modelData.title
                    subtitle: `${modelData.module} · ${modelData.size[0]} × ${modelData.size[1]} · drag it out`
                    leadingSize: 28

                    leading: Symbol {
                        anchors.centerIn: parent
                        name: entry.modelData.icon ?? "grid"
                        size: 18
                        color: Theme.foreground
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                        preventStealing: true
                        onPressed: event => {
                            search.focus = false;
                            root.at = mapToItem(root, event.x, event.y);
                            root.dragging = entry.modelData;
                            root.close();
                        }
                        onPositionChanged: event => root.at = mapToItem(root, event.x, event.y)
                        onReleased: root.drop()
                        onCanceled: root.dragging = null
                    }
                }

                Text {
                    anchors.centerIn: parent
                    visible: list.count === 0
                    text: "No widgets match"
                    color: Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }

            Column {
                id: buttons

                anchors.bottom: parent.bottom
                width: parent.width
                spacing: Theme.spaceSmall

                Text {
                    visible: (root.desktop.layout?.error ?? null) !== null
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.desktop.layout?.error ?? ""
                    color: Theme.danger
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                Row {
                    width: parent.width
                    spacing: Theme.spaceSmall

                    // Copy the layout as widgets.toml; the arrow has the other formats.
                    Row {
                        id: copy

                        width: (parent.width - 8) / 2
                        spacing: 2

                        Button {
                            width: parent.width - more.width - 2
                            text: "Copy"
                            icon: "copy"
                            onClicked: {
                                Daemon.command("widgets", "copy", ["toml"]);
                                light.flash();
                            }
                        }

                        Button {
                            id: more

                            icon: "chevron"
                            iconSize: 12
                            rotation: root.menu ? 270 : 90
                            onClicked: root.menu = !root.menu
                        }
                    }

                    Button {
                        width: (parent.width - 8) / 2
                        text: "Done"
                        icon: "check"
                        tone: "accent"
                        onClicked: Daemon.command("widgets", "edit", ["off"])
                    }
                }
            }

            // The other formats, over the copy button.
            Rectangle {
                visible: root.menu
                anchors.bottom: buttons.top
                anchors.bottomMargin: Theme.spaceSmall
                width: copy.width
                height: formats.implicitHeight + 8
                radius: Theme.radiusField
                color: Theme.raised

                Column {
                    id: formats

                    x: 4
                    y: 4
                    width: parent.width - 8

                    Repeater {
                        model: [
                            {
                                "label": "Copy as TOML",
                                "format": "toml"
                            },
                            {
                                "label": "Copy as Nix",
                                "format": "nix"
                            }
                        ]

                        Rectangle {
                            id: format

                            required property var modelData

                            width: formats.width
                            height: 32
                            radius: Theme.radiusControl
                            color: formatArea.containsMouse ? Theme.highlight : "transparent"

                            Text {
                                x: 10
                                anchors.verticalCenter: parent.verticalCenter
                                text: format.modelData.label
                                color: Theme.foreground
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                            }

                            MouseArea {
                                id: formatArea

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    Daemon.command("widgets", "copy", [format.modelData.format]);
                                    light.flash();
                                    root.menu = false;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // The widget being dragged out, at its size on the grid.
    Rectangle {
        id: ghost

        readonly property real cell: root.desktop.cell

        visible: root.dragging !== null
        width: (root.dragging?.size[0] ?? 0) * cell
        height: (root.dragging?.size[1] ?? 0) * cell
        x: Place.snap(root.at.x - width / 2, cell)
        y: Place.snap(root.at.y - height / 2, cell)
        radius: Theme.radiusSurface
        color: Qt.alpha(Theme.foreground, 0.08)
        border.width: 2
        border.color: Theme.accent

        Text {
            anchors.centerIn: parent
            text: root.dragging?.title ?? ""
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }
    }

    // Placed where it was let go; the drawer folded as the drag began.
    function drop(): void {
        const entry = dragging;
        dragging = null;
        if (!entry)
            return;
        const x = Math.max(0, Math.min(desktop.width - ghost.width, ghost.x));
        const y = Math.max(0, Math.min(desktop.height - ghost.height, ghost.y));
        const spot = Place.place(x, y, ghost.width, ghost.height, ghost.cell, desktop.width, desktop.height);
        Daemon.command("widgets", "add", [entry.module, entry.widget, desktop.output, spot.anchor, `${spot.x}`, `${spot.y}`]);
    }
}

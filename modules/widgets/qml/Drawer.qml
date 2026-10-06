import QtQuick
import Quickshell
import qs.island
import "Place.js" as Place

// While arranging: every widget the running modules offer, to drag onto
// the screen, and the buttons to copy the layout for home-manager or to
// stop arranging.
Item {
    id: root

    required property Item desktop

    anchors.fill: parent

    // What's being dragged out, or null: an entry of the catalog.
    property var dragging: null
    property point at

    // On the side with fewer widgets under it, so it hides as few as it can.
    readonly property bool onLeft: {
        const strip = 280 + 32;
        let right = 0;
        let left = 0;
        for (const widget of desktop.placed) {
            const box = Place.rect(widget, desktop.cell, desktop.width, desktop.height);
            right += Math.max(0, box.x + box.width - (desktop.width - strip)) * box.height;
            left += Math.max(0, strip - box.x) * box.height;
        }
        return left < right;
    }

    Rectangle {
        id: panel

        x: root.onLeft ? 16 : parent.width - width - 16
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.margins: 16
        width: 280
        radius: Theme.radiusLarge
        color: Theme.background
        border.width: 1
        border.color: Theme.border

        // Clicks here stay here.
        MouseArea {
            anchors.fill: parent
        }

        Column {
            id: header

            x: Theme.padding
            y: Theme.padding
            width: parent.width - Theme.padding * 2
            spacing: 4

            Text {
                text: "Widgets"
                color: Theme.foreground
                font.pixelSize: Theme.textHeadline
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }

            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "Drag one onto the screen. Drag a widget to move it, its corner to resize it; click it for its settings."
                color: Theme.muted
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }
        }

        ListView {
            id: list

            anchors.top: header.bottom
            anchors.topMargin: 12
            anchors.bottom: buttons.top
            anchors.bottomMargin: 12
            x: 8
            width: parent.width - 16
            clip: true
            spacing: 4
            model: root.desktop.catalog
            boundsBehavior: Flickable.StopAtBounds

            delegate: ListRow {
                id: entry

                required property var modelData

                width: list.width
                title: modelData.title
                subtitle: `${modelData.module} · ${modelData.size[0]} × ${modelData.size[1]}`
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
                    onPressed: event => {
                        root.dragging = entry.modelData;
                        root.at = mapToItem(root, event.x, event.y);
                    }
                    onPositionChanged: event => root.at = mapToItem(root, event.x, event.y)
                    onReleased: root.drop()
                    onCanceled: root.dragging = null
                    preventStealing: true
                }
            }
        }

        Column {
            id: buttons

            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.padding
            x: Theme.padding
            width: parent.width - Theme.padding * 2
            spacing: 8

            Text {
                visible: (root.desktop.layout?.error ?? null) !== null
                width: parent.width
                wrapMode: Text.Wrap
                text: root.desktop.layout?.error ?? ""
                color: Theme.danger
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }

            Button {
                width: parent.width
                text: "Copy as Nix"
                icon: "copy"
                onClicked: Daemon.command("widgets", "copy", ["nix"])
            }

            Button {
                width: parent.width
                text: "Done"
                icon: "check"
                tone: "accent"
                onClicked: Daemon.command("widgets", "edit", ["off"])
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
        radius: Theme.radiusLarge
        color: Qt.rgba(1, 1, 1, 0.08)
        border.width: 2
        border.color: Theme.accent

        Text {
            anchors.centerIn: parent
            text: root.dragging?.title ?? ""
            color: Theme.foreground
            font.pixelSize: Theme.textSubtitle
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }
    }

    // Placed where it was let go, unless that's over the drawer.
    function drop(): void {
        const entry = dragging;
        dragging = null;
        if (!entry || (at.x >= panel.x && at.x <= panel.x + panel.width))
            return;
        const x = Math.max(0, Math.min(desktop.width - ghost.width, ghost.x));
        const y = Math.max(0, Math.min(desktop.height - ghost.height, ghost.y));
        const spot = Place.place(x, y, ghost.width, ghost.height, ghost.cell, desktop.width, desktop.height);
        Daemon.command("widgets", "add", [entry.module, entry.widget, desktop.output, spot.anchor, `${spot.x}`, `${spot.y}`]);
    }
}

import QtQuick
import qs.island
import "Place.js" as Place

// One monitor's widgets, in the desktop layer the core makes for each
// monitor. It tells that window what takes clicks (`shapes`), what gets a
// blurred card (`framed`) and whether a widget wants the keyboard. While
// arranging on this monitor, it dims the screen, shows the grid and the
// drawer, and every widget can be moved, resized and set up.
Item {
    id: root

    // Set by the window: this monitor's name.
    property string output: ""

    readonly property var layout: Daemon.state("widgets")
    readonly property real cell: layout?.grid ?? 16
    readonly property bool editing: (layout?.editing ?? false) && layout?.output === output
    // Widgets whose module isn't running have no view, and wait.
    readonly property var placed: (layout?.widgets ?? []).filter(widget => widget.output === output && widget.view)
    readonly property var byId: {
        const map = {};
        for (const widget of placed)
            map[widget.id] = widget;
        return map;
    }
    readonly property var catalog: layout?.catalog ?? []

    // For the window.
    property list<Item> shapes: []
    // The island's strip. While arranging, this layer holds the keyboard,
    // and Hyprland only sends the pointer to the surface holding it, so the
    // island never hears a click: this layer takes it there instead, and
    // opens the drawer the island's notice talks about.
    readonly property rect hole: {
        const width = 480;
        const height = Theme.margin + Theme.idleHeight + 6;
        const area = Theme.islandArea;
        const x = area === "left" ? 0 : area === "right" ? root.width - width : area === "center-left" ? root.width / 2 - width : area === "center-right" ? root.width / 2 : (root.width - width) / 2;
        const y = Theme.anchor === "bottom" ? root.height - height : 0;
        return Qt.rect(x, y, width, height);
    }
    property list<Item> framed: []
    property bool wantsKeyboard: false

    // The widget whose settings are open.
    property string selected: ""

    // The alignment guides of the widget being moved or resized:
    // [{x, y, length, axis}], from Place.guides.
    property var guides: []

    // Where the other widgets are, for lining one up with them.
    function others(frame: Item): var {
        const all = [];
        for (let index = 0; index < frames.count; index++) {
            const other = frames.itemAt(index);
            if (other && other !== frame && other.visible)
                all.push(other.placed);
        }
        return all;
    }

    onPlacedChanged: Place.sync(widgets, placed.map(widget => widget.id))
    Component.onCompleted: Place.sync(widgets, placed.map(widget => widget.id))
    onEditingChanged: {
        if (!editing) {
            selected = "";
            return;
        }
        // A widget just sent here from another monitor keeps its settings
        // open.
        selected = layout?.selected ?? "";
        Qt.callLater(() => keys.forceActiveFocus());
    }

    function refreshShapes(): void {
        const all = [];
        const cards = [];
        let typing = false;
        for (let index = 0; index < frames.count; index++) {
            const frame = frames.itemAt(index);
            if (!frame || !frame.visible)
                continue;
            all.push(frame);
            if (frame.framed)
                cards.push(frame);
            typing = typing || frame.typing;
        }
        shapes = all;
        framed = cards;
        wantsKeyboard = typing;
    }

    // Escape stops arranging.
    Item {
        id: keys

        focus: root.editing
        Keys.onEscapePressed: Daemon.command("widgets", "edit", ["off"])
    }

    Rectangle {
        anchors.fill: parent
        color: "black"
        opacity: root.editing ? 0.45 : 0
        visible: opacity > 0

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.move
                easing.type: Easing.OutCubic
            }
        }
    }

    // The grid widgets snap to, as faint dots.
    Canvas {
        id: grid

        anchors.fill: parent
        visible: root.editing
        opacity: 0.18
        onVisibleChanged: requestPaint()
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        onPaint: {
            const context = getContext("2d");
            context.clearRect(0, 0, width, height);
            context.fillStyle = "white";
            const step = root.cell * 2;
            for (let x = step; x < width; x += step) {
                for (let y = step; y < height; y += step)
                    context.fillRect(x - 1, y - 1, 2, 2);
            }
        }
    }

    // A click beside the widgets closes the settings and the drawer.
    MouseArea {
        anchors.fill: parent
        enabled: root.editing
        onClicked: {
            root.selected = "";
            if (root.layout?.drawer)
                Daemon.command("widgets", "drawer", ["off"]);
        }
    }

    // Over the island's notice: opens the drawer.
    MouseArea {
        x: root.hole.x
        y: root.hole.y
        width: root.hole.width
        height: root.hole.height
        z: 1
        enabled: root.editing
        cursorShape: Qt.PointingHandCursor
        onClicked: Daemon.command("widgets", "drawer", ["on"])
    }

    // The widgets, in their own stack: their layers order them among
    // themselves, under the settings and the drawer.
    Item {
        anchors.fill: parent

        Repeater {
            id: frames

            model: ListModel {
                id: widgets
            }
            onItemAdded: Qt.callLater(root.refreshShapes)
            onItemRemoved: Qt.callLater(root.refreshShapes)

            Frame {
                required property string key

                desktop: root
                widget: root.byId[key] ?? null
                onVisibleChanged: Qt.callLater(root.refreshShapes)
                onTypingChanged: Qt.callLater(root.refreshShapes)
                onFramedChanged: Qt.callLater(root.refreshShapes)
            }
        }
    }

    // Thin accent lines where the moving widget lines up, over the widgets.
    Repeater {
        model: root.editing ? root.guides : []

        Rectangle {
            required property var modelData

            x: modelData.axis === 0 ? Math.round(modelData.x) : modelData.x
            y: modelData.axis === 1 ? Math.round(modelData.y) : modelData.y
            width: modelData.axis === 0 ? 1 : modelData.length
            height: modelData.axis === 0 ? modelData.length : 1
            z: 1
            color: Theme.accent
        }
    }

    Settings {
        desktop: root
        frame: {
            for (let index = 0; index < frames.count; index++) {
                const frame = frames.itemAt(index);
                if (frame?.widget?.id === root.selected)
                    return frame;
            }
            return null;
        }
    }

    Drawer {
        desktop: root
        visible: root.editing
    }
}

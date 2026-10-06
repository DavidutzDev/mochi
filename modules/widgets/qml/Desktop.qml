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
    property list<Item> framed: []
    property bool wantsKeyboard: false

    // The widget whose settings are open.
    property string selected: ""

    onPlacedChanged: Place.sync(widgets, placed.map(widget => widget.id))
    Component.onCompleted: Place.sync(widgets, placed.map(widget => widget.id))
    onEditingChanged: {
        if (!editing)
            selected = "";
        else
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

    // A click beside the widgets closes the settings.
    MouseArea {
        anchors.fill: parent
        enabled: root.editing
        onClicked: root.selected = ""
    }

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

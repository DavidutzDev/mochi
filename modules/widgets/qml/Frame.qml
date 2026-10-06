import QtQuick
import QtQuick.Effects
import qs.island
import "Place.js" as Place

// One placed widget: the card behind it, unless it asks for none, and its
// module's view, which gets the module's state as `payload`, its own
// `settings` and its `instance` id. While arranging, dragging it moves it
// on the grid, the corner handle resizes it, and the buttons open its
// settings or remove it. Each change goes to the module, which saves it.
Item {
    id: root

    required property Item desktop
    required property var widget

    readonly property real cell: desktop.cell
    readonly property bool editing: desktop.editing
    readonly property bool framed: (widget?.frame ?? true) && !hidden
    // A view can set `hidden` to step aside, like a card with nothing to show.
    readonly property bool hidden: content.item?.hidden ?? false
    // A view sets `typing` while a text field in it has the focus.
    readonly property bool typing: content.item?.typing ?? false
    readonly property var placed: widget ? Place.rect(widget, cell, desktop.width, desktop.height) : ({ x: 0, y: 0, width: 0, height: 0 })

    // While dragging or resizing, and until the saved layout comes back.
    property bool moving: false
    property real liveX: 0
    property real liveY: 0
    property real liveWidth: 0
    property real liveHeight: 0

    onWidgetChanged: moving = false

    x: moving ? liveX : placed.x
    y: moving ? liveY : placed.y
    width: moving ? liveWidth : placed.width
    height: moving ? liveHeight : placed.height
    visible: widget !== null && (!hidden || editing)

    Behavior on x {
        enabled: !root.moving
        NumberAnimation {
            duration: Theme.move
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Theme.overshoot
        }
    }

    Behavior on y {
        enabled: !root.moving
        NumberAnimation {
            duration: Theme.move
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Theme.overshoot
        }
    }

    function begin(): void {
        liveX = x;
        liveY = y;
        liveWidth = width;
        liveHeight = height;
        moving = true;
    }

    // Saves where it is and how big: the module answers with the new
    // layout, which ends `moving`.
    function commit(): void {
        const spot = Place.place(liveX, liveY, liveWidth, liveHeight, cell, desktop.width, desktop.height);
        Daemon.command("widgets", "move", [widget.id, desktop.output, spot.anchor, `${spot.x}`, `${spot.y}`]);
        const columns = Math.round(liveWidth / cell);
        const rows = Math.round(liveHeight / cell);
        if (columns !== widget.width || rows !== widget.height)
            Daemon.command("widgets", "resize", [widget.id, `${columns}`, `${rows}`]);
    }

    RectangularShadow {
        anchors.fill: card
        visible: root.framed && Theme.shadow.a > 0
        radius: card.radius
        blur: 20
        offset.y: 3
        color: Theme.shadow
    }

    Rectangle {
        id: card

        anchors.fill: parent
        visible: root.framed
        radius: Theme.radiusLarge
        color: Theme.background
        border.width: 1
        border.color: Theme.border
    }

    Loader {
        id: content

        // Changes only when the view does; the rest goes in through the
        // bindings below.
        readonly property string url: root.widget ? `root:/modules/${root.widget.module}/${root.widget.view}.qml` : ""

        anchors.fill: parent
        anchors.margins: root.framed ? Theme.padding : 0
        enabled: !root.editing
        onUrlChanged: {
            if (url === "")
                return;
            setSource(url, {
                payload: Daemon.state(root.widget.module),
                settings: root.widget.settings,
                instance: root.widget.id
            });
        }
    }

    Binding {
        target: content.item
        property: "payload"
        value: Daemon.state(root.widget?.module ?? "")
        when: content.item !== null && root.widget !== null
    }

    Binding {
        target: content.item
        property: "settings"
        value: root.widget?.settings ?? ({})
        when: content.item !== null && root.widget !== null
    }

    // Arranging: an outline, a handle to drag, and buttons.
    Rectangle {
        anchors.fill: parent
        visible: root.editing
        radius: Theme.radiusLarge
        color: "transparent"
        border.width: root.desktop.selected === root.widget?.id ? 2 : 1
        border.color: root.desktop.selected === root.widget?.id ? Theme.accent : Qt.rgba(1, 1, 1, 0.35)
    }

    MouseArea {
        id: drag

        property point start

        anchors.fill: parent
        enabled: root.editing
        hoverEnabled: true
        cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
        onPressed: event => {
            start = mapToItem(root.desktop, event.x, event.y);
            root.begin();
        }
        onPositionChanged: event => {
            if (!pressed)
                return;
            const now = mapToItem(root.desktop, event.x, event.y);
            const x = root.placed.x + now.x - start.x;
            const y = root.placed.y + now.y - start.y;
            root.liveX = Math.max(0, Math.min(root.desktop.width - root.liveWidth, Place.snap(x, root.cell)));
            root.liveY = Math.max(0, Math.min(root.desktop.height - root.liveHeight, Place.snap(y, root.cell)));
        }
        onReleased: {
            if (root.liveX === root.placed.x && root.liveY === root.placed.y) {
                // A click: open its settings.
                root.moving = false;
                root.desktop.selected = root.desktop.selected === root.widget.id ? "" : root.widget.id;
                return;
            }
            root.commit();
        }
    }

    // Resizing, from the bottom-right corner, within the widget's limits.
    Rectangle {
        id: corner

        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: -6
        width: 18
        height: 18
        radius: 9
        visible: root.editing
        color: Theme.foreground
        border.width: 3
        border.color: Theme.background

        MouseArea {
            property point start

            anchors.fill: parent
            anchors.margins: -6
            cursorShape: Qt.SizeFDiagCursor
            onPressed: event => {
                start = mapToItem(root.desktop, event.x, event.y);
                root.begin();
            }
            onPositionChanged: event => {
                if (!pressed)
                    return;
                const now = mapToItem(root.desktop, event.x, event.y);
                const min = root.widget.min ?? [2, 2];
                const max = root.widget.max ?? [200, 120];
                const columns = Math.round((root.placed.width + now.x - start.x) / root.cell);
                const rows = Math.round((root.placed.height + now.y - start.y) / root.cell);
                root.liveWidth = Math.max(min[0], Math.min(max[0], columns)) * root.cell;
                root.liveHeight = Math.max(min[1], Math.min(max[1], rows)) * root.cell;
            }
            onReleased: root.commit()
        }
    }

    Row {
        anchors.right: parent.right
        anchors.bottom: parent.top
        anchors.bottomMargin: 6
        spacing: 4
        visible: root.editing && (drag.containsMouse || root.desktop.selected === root.widget?.id)

        IconButton {
            icon: "edit"
            size: 14
            tone: "neutral"
            onClicked: root.desktop.selected = root.desktop.selected === root.widget.id ? "" : root.widget.id
        }

        IconButton {
            icon: "trash"
            size: 14
            tone: "danger"
            onClicked: Daemon.command("widgets", "remove", [root.widget.id])
        }
    }
}

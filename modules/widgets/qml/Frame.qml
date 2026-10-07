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
    readonly property var placed: widget ? Place.rect(widget, cell, desktop.width, desktop.height) : ({
            x: 0,
            y: 0,
            width: 0,
            height: 0
        })

    // While dragging or resizing, and until the saved layout comes back.
    property bool moving: false
    property real liveX: 0
    property real liveY: 0
    property real liveWidth: 0
    property real liveHeight: 0

    onWidgetChanged: moving = false

    // Its layer; while it moves or its settings are open, over everything.
    z: moving || desktop.selected === widget?.id ? 100000 : (widget?.z ?? 0)
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

    // Within this many pixels, an edge or the middle goes to a guide, before
    // the grid.
    readonly property real reach: 6

    // Puts the widget's top-left corner near x, y while dragging: on a
    // guide where one is in reach and the spot can be saved as it is, else
    // on the grid.
    function moveTo(x: real, y: real): void {
        const others = desktop.others(root);
        const size = [desktop.width, desktop.height];
        const length = [liveWidth, liveHeight];
        const spot = [x, y];
        // On the grid, where it will be saved, so it doesn't jump when let go.
        const saved = Place.saved(x, y, liveWidth, liveHeight, cell, desktop.width, desktop.height);
        const grid = [saved.x, saved.y];
        const live = [0, 0];
        for (const axis of [0, 1]) {
            const shift = Place.guideShift({
                x: x,
                y: y,
                width: liveWidth,
                height: liveHeight
            }, axis, others, size[axis], reach, shift => {
                const moved = spot[axis] + shift;
                if (moved < 0 || moved > size[axis] - length[axis])
                    return false;
                const at = axis === 0 ? [moved, y] : [x, moved];
                return Place.fits(at[0], at[1], liveWidth, liveHeight, cell, desktop.width, desktop.height)[axis];
            });
            live[axis] = shift !== null ? spot[axis] + shift : Math.max(0, Math.min(size[axis] - length[axis], grid[axis]));
        }
        liveX = live[0];
        liveY = live[1];
        showGuides(others);
    }

    // Sizes the widget near w by h pixels while resizing, in whole cells
    // within its limits: its far edge or middle on a guide in reach, else
    // the nearest cell.
    function sizeTo(w: real, h: real): void {
        const others = desktop.others(root);
        const min = widget.min ?? [2, 2];
        const max = widget.max ?? [200, 120];
        const start = [liveX, liveY];
        const size = [desktop.width, desktop.height];
        const wanted = [w, h];
        const live = [0, 0];
        for (const axis of [0, 1]) {
            const length = Place.guideLength(start[axis], wanted[axis], others, axis, size[axis], cell, reach, length => {
                if (length < min[axis] * cell || length > max[axis] * cell)
                    return false;
                const at = axis === 0 ? [length, liveHeight] : [liveWidth, length];
                return Place.fits(liveX, liveY, at[0], at[1], cell, desktop.width, desktop.height)[axis];
            });
            const cells = Math.max(min[axis], Math.min(max[axis], Math.round(wanted[axis] / cell)));
            live[axis] = length ?? cells * cell;
        }
        liveWidth = live[0];
        liveHeight = live[1];
        showGuides(others);
    }

    function showGuides(others: var): void {
        desktop.guides = Place.guides({
            x: liveX,
            y: liveY,
            width: liveWidth,
            height: liveHeight
        }, others, desktop.width, desktop.height);
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
        radius: Theme.radiusSurface
        color: Theme.background
        border.width: 1
        border.color: Theme.border

        EdgeLight {
            radius: card.radius
        }
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
        radius: Theme.radiusSurface
        color: "transparent"
        border.width: root.desktop.selected === root.widget?.id ? 2 : 1
        border.color: root.desktop.selected === root.widget?.id ? Theme.accent : Qt.alpha(Theme.foreground, 0.35)
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
            root.moveTo(root.placed.x + now.x - start.x, root.placed.y + now.y - start.y);
        }
        onReleased: {
            root.desktop.guides = [];
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
        radius: height / 2
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
                root.sizeTo(root.placed.width + now.x - start.x, root.placed.height + now.y - start.y);
            }
            onReleased: {
                root.desktop.guides = [];
                root.commit();
            }
        }
    }

    // Pointed at, the widget or its buttons; it stays a moment after, so the
    // pointer can cross the gap from the widget to the buttons.
    readonly property bool pointed: drag.containsMouse || toolsHover.hovered
    onPointedChanged: {
        if (!pointed)
            linger.restart();
    }

    Timer {
        id: linger

        interval: 500
    }

    Row {
        anchors.right: parent.right
        anchors.bottom: parent.top
        anchors.bottomMargin: Theme.spaceSmall
        spacing: Theme.spaceTiny
        visible: root.editing && (root.pointed || linger.running || root.desktop.selected === root.widget?.id)

        HoverHandler {
            id: toolsHover
        }

        // Layers: over or under the widgets it overlaps.
        IconButton {
            icon: "chevron"
            rotation: 270
            size: 12
            tone: "neutral"
            onClicked: Daemon.command("widgets", "layer", [root.widget.id, "up"])
        }

        IconButton {
            icon: "chevron"
            rotation: 90
            size: 12
            tone: "neutral"
            onClicked: Daemon.command("widgets", "layer", [root.widget.id, "down"])
        }

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

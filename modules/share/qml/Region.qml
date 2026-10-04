import QtQuick
import Quickshell
import qs.island

// Draws the area to share over one live monitor: drag it, and releasing
// shares it, the way the portal's own picker does. Escape or a right click
// goes back to the list. The area goes to the portal on its monitor, in
// logical pixels.
Item {
    id: root

    property var payload: ({})
    property var screen: null

    property var dragged: null

    Component.onCompleted: Qt.callLater(() => keys.forceActiveFocus())

    // The shade around the area being drawn, or over everything before.
    Item {
        anchors.fill: parent
        opacity: 0.4

        readonly property rect cut: root.dragged ?? Qt.rect(0, 0, 0, 0)

        Rectangle {
            width: parent.width
            height: root.dragged ? parent.cut.y : parent.height
            color: "black"
        }

        Rectangle {
            visible: root.dragged !== null
            y: parent.cut.y + parent.cut.height
            width: parent.width
            height: parent.height - y
            color: "black"
        }

        Rectangle {
            visible: root.dragged !== null
            y: parent.cut.y
            width: parent.cut.x
            height: parent.cut.height
            color: "black"
        }

        Rectangle {
            visible: root.dragged !== null
            x: parent.cut.x + parent.cut.width
            y: parent.cut.y
            width: parent.width - x
            height: parent.cut.height
            color: "black"
        }
    }

    Rectangle {
        visible: root.dragged !== null
        x: root.dragged?.x ?? 0
        y: root.dragged?.y ?? 0
        width: root.dragged?.width ?? 0
        height: root.dragged?.height ?? 0
        color: "transparent"
        border.color: Theme.accent
        border.width: 2
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.CrossCursor

        property point start

        onPressed: mouse => {
            if (mouse.button === Qt.RightButton) {
                Daemon.command("share", "back", []);
                return;
            }
            start = Qt.point(mouse.x, mouse.y);
            root.dragged = Qt.rect(mouse.x, mouse.y, 0, 0);
        }

        onPositionChanged: mouse => {
            if (!pressed || root.dragged === null)
                return;
            const x = Math.max(0, Math.min(mouse.x, root.width));
            const y = Math.max(0, Math.min(mouse.y, root.height));
            root.dragged = Qt.rect(Math.min(start.x, x), Math.min(start.y, y), Math.abs(x - start.x), Math.abs(y - start.y));
        }

        onReleased: mouse => {
            const area = root.dragged;
            root.dragged = null;
            if (mouse.button !== Qt.LeftButton || area === null || area.width < 4 || area.height < 4)
                return;
            Daemon.command("share", "area", [root.screen.name, String(Math.round(area.x)), String(Math.round(area.y)), String(Math.round(area.width)), String(Math.round(area.height))]);
        }
    }

    Item {
        id: keys

        Keys.onEscapePressed: Daemon.command("share", "back", [])
    }
}

import QtQuick
import qs.island

// The drawer: every app in the tray, with its icon and name. A click
// activates the app, a right click opens its menu here, a middle click does
// its second action. In a menu, entries with more open as a page of their
// own; Escape or the back arrow goes up a level, then back to the apps.
//
// The keyboard works too: the arrows move between the apps or the
// entries, Enter activates an app or picks an entry, Shift+Enter or the
// Menu key opens an app's menu, and Left goes back up from a submenu. The
// app under the pointer or the arrows shows its tooltip under them.
Item {
    id: root

    property var payload: ({})
    readonly property var items: payload.items ?? []
    readonly property var menu: payload.menu ?? null
    readonly property int columns: 4
    readonly property real tileWidth: 104

    // The submenus opened, by entry id, from the top.
    property var path: []
    readonly property var entries: {
        let level = menu?.entries ?? [];
        for (const id of path) {
            const entry = level.find(entry => entry.id === id);
            if (!entry)
                return [];
            level = entry.children ?? [];
        }
        return level;
    }
    readonly property string heading: {
        let level = menu?.entries ?? [];
        let label = menu?.title ?? "";
        for (const id of path) {
            const entry = level.find(entry => entry.id === id);
            if (!entry)
                break;
            label = entry.label;
            level = entry.children ?? [];
        }
        return label;
    }

    implicitWidth: columns * tileWidth + (columns - 1) * Theme.spaceSmall + Theme.padding * 2
    implicitHeight: column.implicitHeight + Theme.padding * 2

    // A new menu starts at its top. The same menu, changed by its app while
    // it's open, stays on the submenu shown, or the nearest one above it
    // that's still there.
    onMenuChanged: {
        if (menu === null || menu.key !== shownKey) {
            path = [];
            current = -1;
        } else {
            const kept = [];
            let level = menu.entries ?? [];
            for (const id of path) {
                const entry = level.find(entry => entry.id === id);
                if (!entry?.submenu)
                    break;
                kept.push(id);
                level = entry.children ?? [];
            }
            if (kept.length !== path.length)
                path = kept;
            else if (current >= level.length)
                current = -1;
        }
        shownKey = menu?.key ?? "";
    }
    property string shownKey: ""

    // The app or the entry the arrows are on; -1 for none yet.
    property int current: -1
    onPathChanged: current = -1
    readonly property var currentItem: menu === null && current >= 0 ? items[current] ?? null : null
    // The app whose tooltip shows: under the pointer, or else the arrows'.
    property var hovered: null
    readonly property string tooltip: ((hovered ?? currentItem)?.tooltip ?? "").trim()

    focus: true
    Keys.onEscapePressed: back()
    Keys.onPressed: event => {
        const shift = (event.modifiers & Qt.ShiftModifier) !== 0;
        if (menu === null)
            appKey(event, shift);
        else
            menuKey(event);
    }

    // The arrows over the grid of apps.
    function appKey(event: var, shift: bool): void {
        const count = items.length;
        if (count === 0)
            return;
        const step = {
            [Qt.Key_Left]: -1,
            [Qt.Key_Right]: 1,
            [Qt.Key_Up]: -columns,
            [Qt.Key_Down]: columns
        }[event.key];
        if (step !== undefined) {
            current = current < 0 ? 0 : Math.max(0, Math.min(count - 1, current + step));
            event.accepted = true;
        } else if (current >= 0 && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            Daemon.command("tray", shift ? "menu" : "activate", [items[current].key]);
            event.accepted = true;
        } else if (current >= 0 && event.key === Qt.Key_Menu) {
            Daemon.command("tray", "menu", [items[current].key]);
            event.accepted = true;
        }
    }

    // The arrows over a menu's entries, skipping separators and what's off.
    function menuKey(event: var): void {
        const usable = index => {
            const entry = entries[index];
            return entry && !entry.separator && entry.enabled;
        };
        const move = direction => {
            for (let index = current + direction; index >= 0 && index < entries.length; index += direction)
                if (usable(index))
                    return index;
            return current;
        };
        switch (event.key) {
        case Qt.Key_Down:
            current = move(1);
            break;
        case Qt.Key_Up:
            current = current < 0 ? move(1) : move(-1);
            break;
        case Qt.Key_Return:
        case Qt.Key_Enter:
        case Qt.Key_Right:
            if (usable(current) && (event.key !== Qt.Key_Right || entries[current].submenu))
                choose(entries[current]);
            break;
        case Qt.Key_Left:
            back();
            break;
        default:
            return;
        }
        event.accepted = true;
    }

    function back(): void {
        if (path.length > 0)
            path = path.slice(0, -1);
        else if (menu !== null)
            Daemon.command("tray", "open", []);
        else
            Daemon.event("dismiss");
    }

    function choose(entry: var): void {
        if (!entry.enabled || entry.separator)
            return;
        if (entry.submenu) {
            path = path.concat([entry.id]);
            Daemon.command("tray", "submenu", [menu.key, `${entry.id}`]);
        } else {
            Daemon.command("tray", "click", [menu.key, `${entry.id}`]);
        }
    }

    EdgeLight {
        radius: Theme.radiusSurface
    }

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: Theme.spaceSmall

        // The heading: the tray, or the menu's app and submenu.
        PanelHeader {
            width: parent.width
            title: root.menu !== null ? root.heading : "Tray"
            back: root.menu !== null
            onBackClicked: root.back()
        }

        // The apps.
        Text {
            visible: root.menu === null && root.items.length === 0
            width: parent.width
            height: 50
            verticalAlignment: Text.AlignVCenter
            horizontalAlignment: Text.AlignHCenter
            text: "No app has a tray icon"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Grid {
            visible: root.menu === null
            columns: root.columns
            spacing: Theme.spaceSmall

            Repeater {
                model: root.menu === null ? root.items : []

                Rectangle {
                    id: tile

                    required property var modelData

                    width: root.tileWidth
                    height: 86
                    radius: Theme.radiusField
                    required property int index

                    // Unimportant for now, its app says.
                    opacity: tile.modelData.passive ? 0.6 : 1
                    color: area.containsMouse || root.current === index ? Theme.raised : Theme.surface

                    AppIcon {
                        id: tileIcon

                        anchors.horizontalCenter: parent.horizontalCenter
                        y: 14
                        icon: tile.modelData.icon ?? ""
                        size: 32
                    }

                    // An app asking for attention.
                    Rectangle {
                        visible: tile.modelData.attention ?? false
                        anchors.right: tileIcon.right
                        anchors.top: tileIcon.top
                        anchors.margins: -3
                        width: 9
                        height: 9
                        radius: height / 2
                        color: Theme.accent
                    }

                    Text {
                        anchors.top: tileIcon.bottom
                        anchors.topMargin: Theme.spaceSmall
                        x: 6
                        width: parent.width - 12
                        horizontalAlignment: Text.AlignHCenter
                        text: tile.modelData.title ?? ""
                        elide: Text.ElideRight
                        color: Theme.foreground
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                        cursorShape: Qt.PointingHandCursor
                        onContainsMouseChanged: {
                            if (containsMouse)
                                root.hovered = tile.modelData;
                            else if (root.hovered?.key === tile.modelData.key)
                                root.hovered = null;
                        }
                        onClicked: mouse => {
                            const key = tile.modelData.key;
                            if (mouse.button === Qt.RightButton)
                                Daemon.command("tray", "menu", [key]);
                            else if (mouse.button === Qt.MiddleButton)
                                Daemon.command("tray", "secondary", [key]);
                            else
                                Daemon.command("tray", "activate", [key]);
                        }
                    }
                }
            }
        }

        // What the app says about itself, like Discord's unread count.
        Text {
            visible: root.menu === null && root.tooltip !== ""
            width: parent.width
            text: root.tooltip
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
            textFormat: Text.PlainText
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        // The menu.
        Text {
            visible: root.menu !== null && root.entries.length === 0
            width: parent.width
            height: 40
            verticalAlignment: Text.AlignVCenter
            horizontalAlignment: Text.AlignHCenter
            text: "Nothing in this menu"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Flickable {
            id: menuList

            visible: root.menu !== null
            width: parent.width
            height: Math.min(entryColumn.implicitHeight, 420)
            contentHeight: entryColumn.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds

            ScrollFade {
                view: menuList
            }

            Column {
                id: entryColumn

                width: parent.width
                spacing: 2

                Repeater {
                    model: root.menu !== null ? root.entries : []

                    Item {
                        id: row

                        required property var modelData
                        required property int index
                        readonly property bool separator: modelData.separator ?? false

                        width: entryColumn.width
                        height: separator ? 11 : 38

                        Rectangle {
                            visible: row.separator
                            anchors.centerIn: parent
                            width: parent.width - 16
                            height: 1
                            color: Theme.raised
                        }

                        Rectangle {
                            visible: !row.separator
                            anchors.fill: parent
                            radius: Theme.radiusControl
                            color: (entryArea.containsMouse || root.current === row.index) && row.modelData.enabled ? Theme.raised : "transparent"
                            opacity: row.modelData.enabled ? 1 : 0.45

                            // A checkmark or a radio dot, when it has one.
                            Symbol {
                                id: mark

                                x: 10
                                anchors.verticalCenter: parent.verticalCenter
                                visible: (row.modelData.toggle ?? "") !== "" && row.modelData.checked
                                name: row.modelData.toggle === "radio" ? "dot" : "check"
                                size: 14
                                color: Theme.accent
                            }

                            AppIcon {
                                id: entryIcon

                                x: 34
                                anchors.verticalCenter: parent.verticalCenter
                                visible: (row.modelData.icon ?? "") !== ""
                                icon: row.modelData.icon ?? ""
                                size: 16
                            }

                            Text {
                                anchors.left: parent.left
                                anchors.leftMargin: entryIcon.visible ? 58 : 34
                                anchors.right: chevron.left
                                anchors.rightMargin: Theme.spaceSmall
                                anchors.verticalCenter: parent.verticalCenter
                                text: row.modelData.label ?? ""
                                elide: Text.ElideRight
                                textFormat: Text.PlainText
                                color: Theme.foreground
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                            }

                            Symbol {
                                id: chevron

                                anchors.right: parent.right
                                anchors.rightMargin: Theme.spaceSmall
                                anchors.verticalCenter: parent.verticalCenter
                                visible: row.modelData.submenu ?? false
                                name: "chevron"
                                size: 12
                                color: Theme.muted
                            }

                            MouseArea {
                                id: entryArea

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: row.modelData.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                                onClicked: root.choose(row.modelData)
                            }
                        }
                    }
                }
            }
        }
    }
}

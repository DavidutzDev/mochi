import QtQuick
import qs.island

// The drawer: every app in the tray, with its icon and name. A click
// activates the app, a right click opens its menu here, a middle click does
// its second action. In a menu, entries with more open as a page of their
// own; Escape or the back arrow goes up a level, then back to the apps.
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

    implicitWidth: columns * tileWidth + (columns - 1) * 8 + Theme.padding * 2
    implicitHeight: column.implicitHeight + Theme.padding * 2

    // A new menu starts at its top.
    onMenuChanged: {
        if (menu === null || menu.key !== shownKey)
            path = [];
        shownKey = menu?.key ?? "";
    }
    property string shownKey: ""

    focus: true
    Keys.onEscapePressed: back()

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

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: 10

        // The heading: the tray, or the menu's app and submenu.
        Item {
            width: parent.width
            height: 28

            IconButton {
                id: backButton

                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                visible: root.menu !== null
                icon: "chevron"
                rotation: 180
                size: 14
                onClicked: root.back()
            }

            Symbol {
                id: traySymbol

                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                visible: root.menu === null
                name: "tray"
                size: 16
            }

            Text {
                anchors.left: root.menu !== null ? backButton.right : traySymbol.right
                anchors.leftMargin: 8
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: root.menu !== null ? root.heading : "Tray"
                elide: Text.ElideRight
                color: Theme.foreground
                font.pixelSize: Theme.textSubtitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }
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
            spacing: 8

            Repeater {
                model: root.menu === null ? root.items : []

                Rectangle {
                    id: tile

                    required property var modelData

                    width: root.tileWidth
                    height: 86
                    radius: Theme.radiusMedium
                    // Unimportant for now, its app says.
                    opacity: tile.modelData.passive ? 0.6 : 1
                    color: area.containsMouse ? Theme.raised : Theme.surface

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
                        radius: 4.5
                        color: Theme.accent
                    }

                    Text {
                        anchors.top: tileIcon.bottom
                        anchors.topMargin: 8
                        x: 6
                        width: parent.width - 12
                        horizontalAlignment: Text.AlignHCenter
                        text: tile.modelData.title ?? ""
                        elide: Text.ElideRight
                        color: Theme.foreground
                        font.pixelSize: Theme.textLabel
                        font.family: Theme.fontFamily
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                        cursorShape: Qt.PointingHandCursor
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
            visible: root.menu !== null
            width: parent.width
            height: Math.min(entryColumn.implicitHeight, 420)
            contentHeight: entryColumn.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds

            Column {
                id: entryColumn

                width: parent.width
                spacing: 2

                Repeater {
                    model: root.menu !== null ? root.entries : []

                    Item {
                        id: row

                        required property var modelData
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
                            radius: Theme.radiusSmall
                            color: entryArea.containsMouse && row.modelData.enabled ? Theme.raised : "transparent"
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
                                anchors.rightMargin: 8
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
                                anchors.rightMargin: 10
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

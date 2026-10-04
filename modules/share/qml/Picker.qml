import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Widgets
import qs.island

// The island asking what to share: the screens and the windows the portal
// offers, each with a live thumbnail, or an area to draw. Tab switches
// between screens and windows, arrows move, Enter shares, R draws an area,
// Escape shares nothing.
Item {
    id: root

    property var payload: ({})
    property string tab: "screens"
    property int current: 0

    readonly property var windows: payload.windows ?? []
    readonly property int count: tab === "screens" ? Quickshell.screens.length : windows.length
    readonly property int columns: 3
    readonly property real thumbWidth: 220
    readonly property real thumbHeight: 124

    implicitWidth: columns * thumbWidth + (columns - 1) * 10 + Theme.padding * 2
    implicitHeight: column.implicitHeight + Theme.padding * 2

    onTabChanged: current = 0
    Component.onCompleted: Qt.callLater(() => keys.forceActiveFocus())

    // Hyprland's toplevel for a window the portal lists, for its thumbnail.
    function toplevel(address: string): var {
        const bare = value => (value ?? "").replace(/^0x/, "").replace(/^0+/, "").toLowerCase();
        const wanted = bare(address);
        if (!wanted)
            return null;
        return Hyprland.toplevels.values.find(toplevel => bare(toplevel.address) === wanted) ?? null;
    }

    function share(index: int): void {
        if (tab === "screens") {
            const screen = Quickshell.screens[index];
            if (screen)
                Daemon.command("share", "screen", [screen.name]);
        } else {
            const window = windows[index];
            if (window)
                Daemon.command("share", "window", [window.handle]);
        }
    }

    function move(by: int): void {
        if (count > 0)
            current = (current + by + count) % count;
    }

    Item {
        id: keys

        focus: true
        Keys.onLeftPressed: root.move(-1)
        Keys.onRightPressed: root.move(1)
        Keys.onUpPressed: root.move(-root.columns)
        Keys.onDownPressed: root.move(root.columns)
        Keys.onTabPressed: root.tab = root.tab === "screens" ? "windows" : "screens"
        Keys.onBacktabPressed: root.tab = root.tab === "screens" ? "windows" : "screens"
        Keys.onReturnPressed: root.share(root.current)
        Keys.onEnterPressed: root.share(root.current)
        Keys.onEscapePressed: Daemon.command("share", "cancel", [])
        Keys.onPressed: event => {
            if (event.key === Qt.Key_R) {
                Daemon.command("share", "region", []);
                event.accepted = true;
            }
        }
    }

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: 12

        Item {
            width: parent.width
            height: 30

            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "display"
                    size: 16
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Share your screen"
                    color: Theme.foreground
                    font.pixelSize: Theme.textSubtitle
                    font.family: Theme.fontFamily
                    font.weight: Font.DemiBold
                }
            }

            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Remember"
                    color: Theme.muted
                    font.pixelSize: Theme.textLabel
                    font.family: Theme.fontFamily
                }

                Switch {
                    anchors.verticalCenter: parent.verticalCenter
                    checked: root.payload.remember ?? false
                    onToggled: Daemon.command("share", "remember", [])
                }
            }
        }

        Row {
            width: parent.width
            spacing: 10

            Segmented {
                width: parent.width - region.width - parent.spacing
                height: 40
                options: [
                    {
                        "value": "screens",
                        "label": "Screens",
                        "icon": "display"
                    },
                    {
                        "value": "windows",
                        "label": `Windows (${root.windows.length})`,
                        "icon": "window"
                    }
                ]
                current: root.tab
                onPicked: value => root.tab = value
            }

            Button {
                id: region

                anchors.verticalCenter: parent.verticalCenter
                height: 40
                icon: "region"
                text: "Region"
                onClicked: Daemon.command("share", "region", [])
            }
        }

        Text {
            visible: root.count === 0
            width: parent.width
            height: 60
            verticalAlignment: Text.AlignVCenter
            horizontalAlignment: Text.AlignHCenter
            text: "No windows to share"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        // At most three rows; more scroll.
        Flickable {
            width: parent.width
            height: Math.min(grid.implicitHeight, 3 * (root.thumbHeight + 44) + 2 * 10)
            contentHeight: grid.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds

            Grid {
                id: grid

                columns: root.columns
                spacing: 10

                Repeater {
                    model: root.tab === "screens" ? Quickshell.screens : root.windows

                    Thumb {
                        required property var modelData
                        required property int index

                        selected: root.current === index
                        source: root.tab === "screens" ? modelData : root.toplevel(modelData.address)?.wayland ?? null
                        title: root.tab === "screens" ? modelData.name : modelData.title || modelData.class
                        subtitle: root.tab === "screens" ? `${modelData.width} × ${modelData.height}` : modelData.class
                        icon: root.tab === "screens" ? "display" : modelData.class
                        onHovered: root.current = index
                        onClicked: root.share(index)
                    }
                }
            }
        }
    }

    // One screen or window: its live picture, its name below.
    component Thumb: Rectangle {
        id: thumb

        property bool selected: false
        property var source: null
        property string title: ""
        property string subtitle: ""
        property string icon: ""
        signal clicked
        signal hovered

        width: root.thumbWidth
        height: root.thumbHeight + 44
        radius: Theme.radiusLarge
        color: selected ? Theme.raised : Theme.surface
        border.width: selected ? 2 : 0
        border.color: Theme.accent

        ClippingRectangle {
            id: picture

            x: 8
            y: 8
            width: parent.width - 16
            height: root.thumbHeight - 8
            radius: Theme.radiusMedium
            color: Theme.background

            Symbol {
                anchors.centerIn: parent
                visible: !live.hasContent
                name: thumb.icon
                size: 32
                color: Theme.muted
            }

            ScreencopyView {
                id: live

                anchors.fill: parent
                captureSource: thumb.source
                live: true
                constraintSize: Qt.size(width * 2, height * 2)
            }
        }

        Column {
            anchors.top: picture.bottom
            anchors.topMargin: 6
            x: 10
            width: parent.width - 20

            Text {
                width: parent.width
                text: thumb.title
                elide: Text.ElideRight
                color: Theme.foreground
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }

            Text {
                width: parent.width
                text: thumb.subtitle
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onEntered: thumb.hovered()
            onClicked: thumb.clicked()
        }
    }
}

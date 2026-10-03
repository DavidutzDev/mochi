import QtQuick
import qs.island

// The panel: a title, then the home screen's cards or one page, then a
// divider and the navbar. Every card and page comes from a module's
// contribution; this view only lays them out.
Item {
    id: root

    property var payload: ({})
    readonly property var cards: Daemon.offered("hub", "card")
    readonly property var pages: Daemon.offered("hub", "page")
    // "home", or module/id for a page.
    property string page: payload.page ?? "home"
    readonly property var current: pages.find(entry => `${entry.module}/${entry.id}` === page) ?? null
    readonly property var tabs: [{ "module": "", "id": "home", "title": "Home", "icon": "home" }].concat(pages)

    readonly property int margin: 20
    readonly property int columns: 3
    readonly property int gap: 12
    readonly property int cardHeight: 168
    readonly property real column: (width - margin * 2 - gap * (columns - 1)) / columns

    implicitWidth: 880
    implicitHeight: margin + header.height + 12 + body.height + 16 + 1 + navbar.height

    focus: true
    Keys.onEscapePressed: Daemon.event("dismiss")
    Component.onCompleted: Qt.callLater(() => root.forceActiveFocus())

    // One contributed view, with its module's published state as payload.
    component Contributed: Loader {
        id: loader

        required property var entry

        Component.onCompleted: {
            const url = `root:/modules/${entry.module}/${entry.view}.qml`;
            setSource(url, { payload: Daemon.state(entry.module) });
            if (status !== Loader.Ready)
                console.warn(`mochi: could not load ${url}`);
        }

        Binding {
            target: loader.item
            property: "payload"
            value: Daemon.state(loader.entry.module)
            when: loader.item !== null
        }
    }

    Column {
        id: header

        x: root.margin
        y: root.margin
        width: parent.width - root.margin * 2
        spacing: 2

        Text {
            text: root.current?.title ?? "Home"
            color: Theme.foreground
            font.pixelSize: 20
            font.weight: Font.Bold
        }

        Text {
            visible: text !== ""
            text: root.current?.options?.subtitle ?? ""
            color: Theme.muted
            font.pixelSize: 12
        }
    }

    Item {
        id: body

        x: root.margin
        anchors.top: header.bottom
        anchors.topMargin: 12
        width: parent.width - root.margin * 2
        height: root.cardHeight * 2 + root.gap

        Flow {
            anchors.fill: parent
            visible: root.current === null
            spacing: root.gap

            Repeater {
                model: root.cards

                Rectangle {
                    id: card

                    required property var modelData
                    readonly property int span: Math.max(1, Math.min(root.columns, modelData.options?.span ?? 1))

                    width: root.column * span + root.gap * (span - 1)
                    height: root.cardHeight
                    radius: 16
                    color: Theme.surface

                    Row {
                        id: title

                        x: 16
                        y: 14
                        spacing: 7

                        Symbol {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: card.modelData.icon != null
                            name: card.modelData.icon ?? ""
                            size: 14
                            color: Theme.muted
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: card.modelData.title
                            color: Theme.muted
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }
                    }

                    Contributed {
                        entry: card.modelData
                        anchors.fill: parent
                        anchors.margins: 16
                        anchors.topMargin: title.y + title.height + 12
                    }
                }
            }
        }

        // A one-item model, so picking another page builds it fresh.
        Repeater {
            model: root.current ? [root.current] : []

            Contributed {
                required property var modelData

                entry: modelData
                width: body.width
                height: body.height
            }
        }
    }

    Rectangle {
        id: divider

        x: root.margin
        anchors.top: body.bottom
        anchors.topMargin: 16
        width: parent.width - root.margin * 2
        height: 1
        color: Theme.surface
    }

    // Like the workspace dots: icons in circles, the current one stretched
    // into a white pill with its name.
    Item {
        id: navbar

        anchors.top: divider.bottom
        width: parent.width
        height: 64

        Row {
            anchors.centerIn: parent
            spacing: 6

            Repeater {
                model: root.tabs

                Rectangle {
                    id: tab

                    required property var modelData
                    readonly property string key: modelData.module ? `${modelData.module}/${modelData.id}` : "home"
                    readonly property bool selected: root.page === key

                    width: selected ? content.implicitWidth + 32 : height
                    height: 38
                    radius: height / 2
                    color: selected ? Theme.foreground : area.containsMouse ? Theme.surface : "transparent"

                    Behavior on width {
                        NumberAnimation {
                            duration: 220
                            easing.type: Easing.OutCubic
                        }
                    }

                    Behavior on color {
                        ColorAnimation {
                            duration: 160
                        }
                    }

                    Row {
                        id: content

                        anchors.centerIn: parent
                        spacing: 8

                        Symbol {
                            anchors.verticalCenter: parent.verticalCenter
                            name: tab.modelData.icon ?? ""
                            size: 18
                            color: tab.selected ? Theme.background : Theme.muted
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: tab.selected
                            text: tab.modelData.title
                            color: Theme.background
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.page = tab.key
                    }
                }
            }
        }
    }
}

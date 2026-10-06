import QtQuick
import QtQuick.Window
import qs.island

// The panel, laid out like a control center: the home screen's cards as
// labeled sections, or one page, then a divider and the navbar. Every page
// gets the same size, the hub's `width` and `height` settings, capped by the
// screen: a shorter page leaves room below, a longer one scrolls, and the
// navbar never moves. A card whose view sets `hidden` to
// true, like Bluetooth without an adapter, leaves no gap. Every card and
// page comes from a module's contribution; this view only lays them out.
Item {
    id: root

    property var payload: ({})
    readonly property var cards: Daemon.offered("hub", "card")
    readonly property var pages: Daemon.offered("hub", "page")
    // "home", or module/id for a page.
    property string page: payload.page ?? "home"
    // `mochi ipc hub open <page>` while it's open switches pages, after a
    // click on a tab replaced the binding above.
    onPayloadChanged: page = payload.page ?? "home"
    readonly property var current: pages.find(entry => `${entry.module}/${entry.id}` === page) ?? null
    // A module whose state says it isn't `available`, like Bluetooth without
    // an adapter, keeps its page out of the navbar.
    readonly property var tabs: [{ "module": "", "id": "home", "title": "Home", "icon": "home" }].concat(pages.filter(entry => Daemon.state(entry.module)?.available !== false))

    readonly property int margin: 14
    readonly property int columns: 3
    readonly property int gap: 10
    readonly property real column: (width - margin * 2 - gap * (columns - 1)) / columns
    // Room left for the cards or the page once the navbar, the margins and
    // the space around the island are taken.
    readonly property real tallest: Math.max(240, (Screen.height > 0 ? Screen.height : 1080) - 220)
    readonly property real fixedHeight: Math.min(payload.height ?? 480, tallest)

    implicitWidth: payload.width ?? 860
    implicitHeight: margin + body.height + 12 + 1 + navbar.height

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

    // The same height for every page; what's taller scrolls.
    Flickable {
        id: body

        x: root.margin
        y: root.margin
        width: parent.width - root.margin * 2
        contentWidth: width
        contentHeight: root.current ? pageHeight : cards.height
        height: root.fixedHeight
        interactive: contentHeight > height
        boundsBehavior: Flickable.StopAtBounds
        clip: true

        // Set by the page itself: a repeater's items aren't bindable.
        property real pageHeight: 0

        // A new page starts at its top.
        Connections {
            target: root

            function onPageChanged() {
                body.contentY = 0;
            }
        }

        // Each card is a section, like a control center: a small label,
        // then the module's view on a surface. Cards fill rows in order, and
        // one that doesn't fit the row goes to the first row below with
        // room, so wide cards leave no gaps.
        Item {
            id: cards

            width: parent.width
            visible: root.current === null

            function relayout(): void {
                const rows = [];
                let y = 0;
                for (let index = 0; index < placed.count; index++) {
                    const card = placed.itemAt(index);
                    if (!card || !card.visible)
                        continue;
                    let row = rows.find(row => row.free >= card.span);
                    if (!row) {
                        row = { "free": root.columns, "items": [], "height": 0 };
                        rows.push(row);
                    }
                    row.free -= card.span;
                    row.items.push(card);
                    row.height = Math.max(row.height, card.height);
                }
                for (const row of rows) {
                    // Free columns go to the row's last card, so no row
                    // ends in a gap.
                    row.items.forEach((card, index) => card.extra = index === row.items.length - 1 ? row.free : 0);
                    let column = 0;
                    for (const card of row.items) {
                        card.x = column * (root.column + root.gap);
                        column += card.span + card.extra;
                        card.y = y;
                    }
                    y += row.height + root.gap;
                }
                height = Math.max(0, y - root.gap);
            }

            onWidthChanged: Qt.callLater(relayout)

            Repeater {
                id: placed

                model: root.cards
                onItemAdded: Qt.callLater(cards.relayout)
                onItemRemoved: Qt.callLater(cards.relayout)

                Column {
                    id: card

                    required property var modelData
                    readonly property int span: Math.max(1, Math.min(root.columns, modelData.options?.span ?? 1))
                    // Columns left free in its row, which it fills.
                    property int extra: 0
                    // The page it opens: `options.page` names one of its
                    // module's pages, otherwise the module's first.
                    readonly property var opens: root.tabs.find(tab => tab.module === modelData.module && (modelData.options?.page == null || tab.id === modelData.options.page)) ?? null

                    width: root.column * (span + extra) + root.gap * (span + extra - 1)
                    // A card with nothing to show hides, and the rest close up.
                    visible: !(view.item?.hidden ?? false)
                    spacing: 6
                    onVisibleChanged: Qt.callLater(cards.relayout)
                    onHeightChanged: Qt.callLater(cards.relayout)

                    // The heading opens the card's page, when it has one.
                    Item {
                        implicitWidth: label.implicitWidth
                        implicitHeight: label.implicitHeight

                        MouseArea {
                            id: heading

                            anchors.fill: parent
                            enabled: card.opens !== null
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.page = `${card.opens.module}/${card.opens.id}`
                        }

                        Row {
                            id: label

                            spacing: 6
                            opacity: heading.containsMouse ? 0.75 : 1

                            Behavior on opacity {
                                NumberAnimation {
                                    duration: Theme.fast
                                }
                            }

                            Symbol {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: card.modelData.icon != null
                                name: card.modelData.icon ?? ""
                                size: 13
                                color: Theme.muted
                            }

                            SectionLabel {
                                anchors.verticalCenter: parent.verticalCenter
                                text: card.modelData.title
                            }

                            Symbol {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: card.opens !== null
                                name: "chevron"
                                size: 10
                                color: Theme.muted
                            }
                        }
                    }

                    Rectangle {
                        width: parent.width
                        height: Math.max(view.height, 52) + 24
                        radius: Theme.radiusLarge
                        color: Theme.surface

                        // A click on the card that misses its controls opens
                        // the page too.
                        MouseArea {
                            anchors.fill: parent
                            enabled: card.opens !== null
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.page = `${card.opens.module}/${card.opens.id}`
                        }

                        Contributed {
                            id: view

                            x: 12
                            y: 12
                            width: parent.width - 24
                            height: item ? item.implicitHeight : 0
                            entry: card.modelData
                        }
                    }
                }
            }
        }

        // A one-item model, so picking another page builds it fresh.
        Repeater {
            id: pageView

            model: root.current ? [root.current] : []

            Contributed {
                id: page

                required property var modelData

                entry: modelData
                width: body.width
                height: item ? item.implicitHeight : 0

                Binding {
                    target: body
                    property: "pageHeight"
                    value: page.height
                }
            }
        }
    }

    Rectangle {
        id: divider

        x: root.margin
        anchors.top: body.bottom
        anchors.topMargin: 12
        width: parent.width - root.margin * 2
        height: 1
        color: Theme.raised
    }

    // Like the workspace dots: icons in circles, the current one stretched
    // into a white pill with its name.
    Item {
        id: navbar

        anchors.top: divider.bottom
        width: parent.width
        height: 52

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

                    width: selected ? content.implicitWidth + 28 : height
                    height: 34
                    radius: height / 2
                    color: selected ? Theme.foreground : area.containsMouse ? Theme.raised : "transparent"

                    Behavior on width {
                        NumberAnimation {
                            duration: Theme.move
                            easing.type: Easing.BezierSpline
                            easing.bezierCurve: Theme.overshoot
                        }
                    }

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.fast
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
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
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

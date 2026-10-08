import QtQuick
import QtQuick.Window
import qs.island

// The panel, laid out like a control center: the home screen's cards on a
// grid of equal rows, or one page, then a divider and the navbar. The hub
// takes the height its content needs, up to its `height` setting and the
// screen, and the island's outline follows it from page to page; what's
// taller scrolls. Each card sits in a frame the hub draws, with its icon,
// title and a chevron inside when it opens a page. A card spans
// `options.span` columns and `options.rows` rows, and its view is sized to
// fill them; without `rows`, it gets as many rows (one or two) as its view
// needs. A card whose view sets `hidden` to true, like Bluetooth without an
// adapter, leaves no gap. Every card and page comes from a module's
// contribution; this view only lays them out.
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
    readonly property var tabs: [
        {
            "module": "",
            "id": "home",
            "title": "Home",
            "icon": "home"
        }
    ].concat(pages.filter(entry => Daemon.state(entry.module)?.available !== false))

    readonly property int margin: Theme.spaceLarge
    readonly property int columns: 3
    readonly property int gap: Theme.spaceMedium
    readonly property real column: (width - margin * 2 - gap * (columns - 1)) / columns
    // A card's frame: the padding around its content, and the heading's
    // height with the space under it.
    readonly property int inset: Theme.spaceMedium
    readonly property int heading: Theme.textCaption + Theme.spaceSmall * 2
    // The most the cards or the page may take once the navbar, the margins
    // and the space around the island are taken.
    readonly property real tallest: Math.max(240, Math.min(payload.height ?? 480, (Screen.height > 0 ? Screen.height : 1080) - 220))

    implicitWidth: payload.width ?? 860
    implicitHeight: margin + body.height + Theme.spaceMedium + 1 + navbar.height

    focus: true
    Keys.onEscapePressed: Daemon.event("dismiss")
    Component.onCompleted: Qt.callLater(() => root.forceActiveFocus())

    // One contributed view, with its module's published state as payload.
    component Contributed: Loader {
        id: loader

        required property var entry

        Component.onCompleted: {
            const url = `root:/modules/${entry.module}/${entry.view}.qml`;
            setSource(url, {
                payload: Daemon.state(entry.module)
            });
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

    // As tall as what it shows, up to `tallest`; what's taller scrolls.
    Flickable {
        id: body

        x: root.margin
        y: root.margin
        width: parent.width - root.margin * 2
        contentWidth: width
        contentHeight: root.current ? pageHeight : cards.height
        height: Math.min(contentHeight, root.tallest)
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

        ScrollFade {
            view: body
        }

        // The cards, in order, each in the first place on the grid where
        // its columns and rows are free, so wide and tall cards leave no
        // gaps.
        Item {
            id: cards

            width: parent.width
            visible: root.current === null

            function relayout(): void {
                // Taken cells, by row.
                const taken = [];
                const free = (row, column, span, rows) => {
                    for (let r = row; r < row + rows; r++)
                        for (let c = column; c < column + span; c++)
                            if (taken[r]?.[c])
                                return false;
                    return true;
                };
                let bottom = 0;
                for (let index = 0; index < placed.count; index++) {
                    const card = placed.itemAt(index);
                    if (!card || !card.visible)
                        continue;
                    let row = 0;
                    let column = 0;
                    search: for (row = 0; ; row++) {
                        for (column = 0; column + card.span <= root.columns; column++)
                            if (free(row, column, card.span, card.rows))
                                break search;
                    }
                    for (let r = row; r < row + card.rows; r++) {
                        taken[r] = taken[r] ?? [];
                        for (let c = column; c < column + card.span; c++)
                            taken[r][c] = true;
                    }
                    card.x = column * (root.column + root.gap);
                    card.y = row * (Theme.tileHeight + root.gap);
                    bottom = Math.max(bottom, card.y + card.height);
                }
                height = bottom;
            }

            onWidthChanged: Qt.callLater(relayout)

            Repeater {
                id: placed

                model: root.cards
                onItemAdded: Qt.callLater(cards.relayout)
                onItemRemoved: Qt.callLater(cards.relayout)

                Rectangle {
                    id: card

                    required property var modelData
                    readonly property int span: Math.max(1, Math.min(root.columns, modelData.options?.span ?? 1))
                    // Declared, or as many rows as the view needs, one or two.
                    readonly property int rows: {
                        const declared = modelData.options?.rows;
                        if (declared != null)
                            return Math.max(1, Math.min(2, declared));
                        const needed = root.inset * 2 + root.heading + view.height;
                        return needed > Theme.tileHeight ? 2 : 1;
                    }
                    // The page it opens: `options.page` names one of its
                    // module's pages, otherwise the module's first.
                    readonly property var opens: root.tabs.find(tab => tab.module === modelData.module && (modelData.options?.page == null || tab.id === modelData.options.page)) ?? null

                    width: root.column * span + root.gap * (span - 1)
                    height: Theme.tileHeight * rows + root.gap * (rows - 1)
                    radius: Theme.radiusSurface
                    color: Theme.surface
                    clip: true
                    // A card with nothing to show hides, and the rest close up.
                    visible: !(view.item?.hidden ?? false)
                    onVisibleChanged: Qt.callLater(cards.relayout)
                    onRowsChanged: Qt.callLater(cards.relayout)

                    EdgeLight {
                        radius: card.radius
                    }

                    // A click on the card that misses its controls, or on its
                    // heading, opens its page.
                    MouseArea {
                        id: area

                        anchors.fill: parent
                        enabled: card.opens !== null
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.page = `${card.opens.module}/${card.opens.id}`
                    }

                    Row {
                        id: heading

                        x: root.inset
                        y: root.inset
                        width: parent.width - root.inset * 2
                        height: Theme.textCaption + Theme.spaceTiny
                        spacing: Theme.spaceTiny

                        Symbol {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: card.modelData.icon != null
                            name: card.modelData.icon ?? ""
                            size: Theme.textBody
                            color: Theme.muted
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: card.modelData.title
                            color: Theme.muted
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightLabel
                        }
                    }

                    Symbol {
                        anchors.right: parent.right
                        anchors.rightMargin: root.inset
                        anchors.verticalCenter: heading.verticalCenter
                        visible: card.opens !== null
                        name: "chevron"
                        size: Theme.textBody
                        color: area.containsMouse ? Theme.foreground : Theme.muted
                    }

                    Contributed {
                        id: view

                        x: root.inset
                        y: root.inset + root.heading
                        width: parent.width - root.inset * 2
                        // A card that declares its rows gets the room they
                        // leave, to fill; one that doesn't is measured.
                        height: card.modelData.options?.rows != null ? card.height - y - root.inset : item ? item.implicitHeight : 0
                        entry: card.modelData
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
        anchors.topMargin: Theme.spaceMedium
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

        // The settings, at the page's module when a page is open.
        IconButton {
            anchors.right: parent.right
            anchors.rightMargin: root.margin
            anchors.verticalCenter: parent.verticalCenter
            visible: Daemon.modules.includes("settings")
            icon: "settings"
            onClicked: Daemon.command("settings", "open", root.current ? [root.current.module] : [])
        }

        Row {
            anchors.centerIn: parent
            spacing: Theme.spaceSmall

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
                        spacing: Theme.spaceSmall

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
                            font.weight: Theme.weightTitle
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

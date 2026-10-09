import QtQuick
import qs.island
import "Place.js" as Place

// While arranging, a panel down the left side of the screen, or the right
// when the island sits on the left, opened from the island's notice. Three
// tabs: Add, every look of every widget the running modules offer, each
// with a live preview, to drag onto the desktop or click into the first
// free spot; On desktop, the widgets placed, to open their settings or
// remove them; and Layouts, arrangements saved under a name. Copy and Done
// at the bottom. The panel slides away while a widget is dragged out of it.
Item {
    id: root

    required property Item desktop

    anchors.fill: parent

    // The look being dragged out, or null: one of `looks`.
    property var dragging: null
    property point at
    property string query: ""
    // The category the list is narrowed to, or "" for all.
    property string category: ""
    property string tab: "add"
    // The saved layout whose Delete waits for a second click.
    property string confirming: ""

    readonly property bool open: desktop.editing && (desktop.layout?.drawer ?? false)
    readonly property bool shown: open && dragging === null
    // Away from the island's notice, which opens it.
    readonly property bool onRight: Theme.islandArea === "left"
    readonly property real gap: Theme.spaceMedium
    // How much of each side it covers, for placing widgets clear of it.
    readonly property real coveredLeft: shown && !onRight ? panel.width + gap : 0
    readonly property real coveredRight: shown && onRight ? panel.width + gap : 0

    // Every look of every widget: one per variant, or the widget itself.
    readonly property var looks: {
        const all = [];
        for (const entry of desktop.catalog) {
            const variants = (entry.variants ?? []).length > 0 ? entry.variants : [null];
            for (const variant of variants) {
                // "Digital" under Clock, "Now playing · Wavy" under Media.
                const plain = variant === null || entry.title.toLowerCase() === entry.category.toLowerCase();
                all.push({
                    entry: entry,
                    variant: variant,
                    key: variant ? `${entry.widget}:${variant.id}` : entry.widget,
                    count: `${entry.module}/${entry.widget}/${variant?.id ?? ""}`,
                    title: plain ? (variant?.title ?? entry.title) : `${entry.title} · ${variant.title}`,
                    description: variant?.description || entry.description || "",
                    category: entry.category
                });
            }
        }
        return all;
    }
    readonly property var categories: [...new Set(looks.map(look => look.category))].sort()
    readonly property var words: query.toLowerCase().split(/\s+/).filter(word => word !== "")
    function matches(text: string): bool {
        const lower = text.toLowerCase();
        return words.every(word => lower.includes(word));
    }
    // The looks shown, by category.
    readonly property var groups: {
        const shown = looks.filter(look => (category === "" || look.category === category) && matches(`${look.title} ${look.description} ${look.category} ${look.entry.module}`));
        return categories.map(name => ({
                    name: name,
                    looks: shown.filter(look => look.category === name)
                })).filter(group => group.looks.length > 0);
    }
    // How many of each look are placed, on every monitor.
    readonly property var placedCounts: {
        const counts = {};
        for (const widget of desktop.layout?.widgets ?? []) {
            const key = `${widget.module}/${widget.widget}/${widget.variant ?? ""}`;
            counts[key] = (counts[key] ?? 0) + 1;
        }
        return counts;
    }
    // Every widget placed, this monitor's first.
    readonly property var placed: {
        const all = (desktop.layout?.widgets ?? []).filter(widget => matches(`${widget.title ?? widget.widget} ${widget.variantTitle ?? ""} ${widget.output}`));
        return all.filter(widget => widget.output === desktop.output).concat(all.filter(widget => widget.output !== desktop.output));
    }
    readonly property var layouts: desktop.layout?.layouts ?? []
    readonly property string current: desktop.layout?.layout ?? ""

    function close(): void {
        copy.open = false;
        Daemon.command("widgets", "drawer", ["off"]);
    }

    // Into the first free spot on this monitor, clear of the panel.
    function add(look: var): void {
        Daemon.command("widgets", "add", [look.entry.module, look.key, desktop.output]);
        light.flash();
    }

    function step(by: int): void {
        const order = ["add", "placed", "layouts"];
        tab = order[(order.indexOf(tab) + by + order.length) % order.length];
    }

    // Scrolls a card that took the keyboard's focus into view.
    function reveal(item: Item): void {
        const top = item.mapToItem(cards.contentItem, 0, 0).y;
        if (top < cards.contentY)
            cards.contentY = top;
        else if (top + item.height > cards.contentY + cards.height)
            cards.contentY = top + item.height - cards.height;
    }

    function saveLayout(): void {
        if (name.text.trim() === "")
            return;
        Daemon.command("widgets", "save-layout", [name.text.trim()]);
        name.text = "";
    }

    // The keyboard goes to the search as it opens, and back to the desktop,
    // where Escape stops arranging, as it closes.
    onOpenChanged: {
        confirming = "";
        if (open)
            Qt.callLater(() => search.forceActiveFocus());
        else if (desktop.editing)
            desktop.takeKeys();
    }

    Rectangle {
        id: panel

        readonly property real hiddenX: root.onRight ? root.width + root.gap : -width - root.gap

        x: root.shown ? (root.onRight ? root.width - width - root.gap : root.gap) : hiddenX
        y: root.gap
        width: Math.min(400, root.width * 0.42)
        height: root.height - root.gap * 2
        // Kept while a card is dragged out, which holds the pointer.
        visible: x !== hiddenX || root.dragging !== null
        radius: Theme.radiusSurface
        // Opaque: the widgets under it would show through the island's
        // see-through background, with no blur behind the panel.
        color: Qt.alpha(Theme.background, 1)
        border.width: 1
        border.color: Theme.border
        clip: true

        Behavior on x {
            NumberAnimation {
                duration: Theme.move
                easing.type: Easing.OutCubic
            }
        }

        // Clicks and hovers here stay here, away from the widgets under it.
        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
        }

        // Flashes when a widget is added or the layout copied.
        EdgeLight {
            id: light

            radius: panel.radius
        }

        // Escape clears the search first, then closes the panel.
        Keys.onEscapePressed: root.close()

        Column {
            id: header

            x: Theme.padding
            y: Theme.padding
            width: parent.width - Theme.padding * 2
            spacing: Theme.spaceMedium

            Column {
                width: parent.width
                spacing: Theme.spaceTiny

                PanelHeader {
                    width: parent.width
                    title: "Widgets"
                    icon: "dashboard_customize"

                    ActionButton {
                        icon: "close"
                        tone: "ghost"
                        onClicked: root.close()
                    }
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.tab === "add" ? "Drag one onto the desktop, or click it to add it." : root.tab === "placed" ? "Every widget placed, on every monitor." : "Arrangements kept under a name, to switch between."
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }
            }

            // The tabs; arrows switch them while they have the focus.
            Item {
                id: tabs

                width: parent.width
                height: Theme.rowHeight - Theme.spaceSmall
                activeFocusOnTab: true
                Keys.onLeftPressed: root.step(-1)
                Keys.onRightPressed: root.step(1)

                Segmented {
                    anchors.fill: parent
                    color: Theme.raised
                    current: root.tab
                    options: [
                        {
                            "value": "add",
                            "label": "Add",
                            "icon": "plus"
                        },
                        {
                            "value": "placed",
                            "label": "On desktop",
                            "icon": "display"
                        },
                        {
                            "value": "layouts",
                            "label": "Layouts",
                            "icon": "bookmarks"
                        }
                    ]
                    onPicked: value => root.tab = value
                }

                Rectangle {
                    visible: tabs.activeFocus
                    anchors.fill: parent
                    anchors.margins: -3
                    radius: height / 2
                    color: "transparent"
                    border.width: 2
                    border.color: Theme.accent
                }
            }

            Rectangle {
                visible: root.tab !== "layouts"
                width: parent.width
                height: Theme.controlHeight + Theme.spaceTiny
                radius: height / 2
                color: Theme.raised
                border.width: search.activeFocus ? 1 : 0
                border.color: Theme.accent

                Symbol {
                    id: magnifier

                    x: Theme.spaceMedium
                    anchors.verticalCenter: parent.verticalCenter
                    name: "search"
                    size: 14
                    color: Theme.muted
                }

                TextInput {
                    id: search

                    anchors.left: magnifier.right
                    anchors.leftMargin: Theme.spaceSmall
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceMedium
                    anchors.verticalCenter: parent.verticalCenter
                    activeFocusOnTab: true
                    color: Theme.foreground
                    selectionColor: Theme.accent
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                    clip: true
                    onTextChanged: root.query = text
                    Keys.onEscapePressed: event => {
                        if (text !== "") {
                            text = "";
                            return;
                        }
                        event.accepted = false;
                    }

                    Text {
                        visible: search.text === ""
                        text: root.tab === "add" ? "Search widgets" : "Search the widgets placed"
                        color: Theme.muted
                        font: search.font
                    }
                }
            }

            // All, and one chip per category.
            Flow {
                visible: root.tab === "add"
                width: parent.width
                spacing: Theme.spaceSmall

                Repeater {
                    model: [""].concat(root.categories)

                    Rectangle {
                        id: chip

                        required property string modelData
                        readonly property bool chosen: root.category === modelData

                        width: label.implicitWidth + Theme.spaceLarge + Theme.spaceTiny
                        height: Theme.controlHeight - Theme.spaceTiny
                        radius: height / 2
                        color: chosen ? Theme.foreground : chipArea.containsMouse ? Theme.highlight : Theme.raised
                        activeFocusOnTab: true
                        Keys.onReturnPressed: root.category = modelData
                        Keys.onEnterPressed: root.category = modelData
                        Keys.onSpacePressed: root.category = modelData

                        Text {
                            id: label

                            anchors.centerIn: parent
                            text: chip.modelData === "" ? "All" : chip.modelData
                            color: chip.chosen ? Theme.background : Theme.foreground
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                        }

                        MouseArea {
                            id: chipArea

                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.category = chip.modelData
                        }

                        Rectangle {
                            visible: chip.activeFocus
                            anchors.fill: parent
                            anchors.margins: -3
                            radius: height / 2
                            color: "transparent"
                            border.width: 2
                            border.color: Theme.accent
                        }
                    }
                }
            }
        }

        // The tab's content, between the header and the buttons.
        Item {
            id: body

            x: Theme.padding
            anchors.top: header.bottom
            anchors.topMargin: Theme.spaceMedium
            anchors.bottom: footer.top
            anchors.bottomMargin: Theme.spaceMedium
            width: parent.width - Theme.padding * 2

            // Add: a card for each look, with its live preview.
            Flickable {
                id: cards

                anchors.fill: parent
                visible: root.tab === "add"
                clip: true
                contentHeight: groups.implicitHeight
                boundsBehavior: Flickable.StopAtBounds

                ScrollFade {
                    view: cards
                }

                Column {
                    id: groups

                    width: cards.width
                    spacing: Theme.spaceLarge

                    Repeater {
                        model: root.groups

                        Column {
                            id: group

                            required property var modelData

                            width: groups.width
                            spacing: Theme.spaceSmall

                            SectionLabel {
                                text: group.modelData.name
                            }

                            Grid {
                                id: grid

                                width: parent.width
                                columns: 2
                                spacing: Theme.spaceSmall

                                Repeater {
                                    model: group.modelData.looks

                                    Rectangle {
                                        id: card

                                        required property var modelData
                                        readonly property int count: root.placedCounts[modelData.count] ?? 0
                                        readonly property bool hovered: cardArea.containsMouse

                                        width: (grid.width - grid.spacing) / 2
                                        height: lines.implicitHeight + Theme.spaceSmall * 2
                                        radius: Theme.radiusField
                                        color: hovered || activeFocus ? Theme.raised : Theme.surface
                                        activeFocusOnTab: true
                                        onActiveFocusChanged: {
                                            if (activeFocus)
                                                root.reveal(card);
                                        }
                                        Keys.onReturnPressed: root.add(modelData)
                                        Keys.onEnterPressed: root.add(modelData)
                                        Keys.onSpacePressed: root.add(modelData)

                                        Behavior on color {
                                            ColorAnimation {
                                                duration: Theme.fast
                                            }
                                        }

                                        Column {
                                            id: lines

                                            x: Theme.spaceSmall
                                            y: Theme.spaceSmall
                                            width: parent.width - Theme.spaceSmall * 2
                                            spacing: Theme.spaceTiny

                                            Preview {
                                                width: parent.width
                                                height: Theme.tileHeight
                                                entry: card.modelData.entry
                                                variant: card.modelData.variant
                                                cell: root.desktop.cell
                                            }

                                            Item {
                                                width: 1
                                                height: Theme.spaceTiny
                                            }

                                            Text {
                                                width: parent.width
                                                elide: Text.ElideRight
                                                text: card.modelData.title
                                                color: Theme.foreground
                                                font.pixelSize: Theme.textBody
                                                font.family: Theme.fontFamily
                                                font.weight: Theme.weightTitle
                                            }

                                            // Two lines, kept even for a short one, so
                                            // cards side by side line up.
                                            Text {
                                                width: parent.width
                                                height: twoLines.implicitHeight
                                                wrapMode: Text.Wrap
                                                maximumLineCount: 2
                                                elide: Text.ElideRight
                                                text: card.modelData.description
                                                color: Theme.muted
                                                font.pixelSize: Theme.textCaption
                                                font.family: Theme.fontFamily

                                                Text {
                                                    id: twoLines

                                                    visible: false
                                                    text: "\n"
                                                    font: parent.font
                                                }
                                            }
                                        }

                                        // How many are on the desktop, over the
                                        // preview's corner.
                                        Rectangle {
                                            anchors.right: parent.right
                                            anchors.top: parent.top
                                            anchors.margins: Theme.spaceTiny
                                            visible: card.count > 0
                                            width: countLabel.implicitWidth + Theme.spaceMedium
                                            height: countLabel.implicitHeight + Theme.spaceTiny
                                            radius: height / 2
                                            color: Theme.background
                                            border.width: 1
                                            border.color: Theme.border

                                            Text {
                                                id: countLabel

                                                anchors.centerIn: parent
                                                text: `${card.count} on the desktop`
                                                color: Theme.foreground
                                                font.pixelSize: Theme.textCaption
                                                font.family: Theme.fontFamily
                                                font.weight: Theme.weightLabel
                                            }
                                        }

                                        // A click adds it; a drag takes it out.
                                        MouseArea {
                                            id: cardArea

                                            property point start
                                            property bool dragged: false

                                            anchors.fill: parent
                                            hoverEnabled: true
                                            preventStealing: true
                                            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.PointingHandCursor
                                            onPressed: event => {
                                                start = Qt.point(event.x, event.y);
                                                dragged = false;
                                            }
                                            onPositionChanged: event => {
                                                if (!pressed)
                                                    return;
                                                if (!dragged && Math.hypot(event.x - start.x, event.y - start.y) > Qt.styleHints.startDragDistance) {
                                                    dragged = true;
                                                    search.focus = false;
                                                    root.desktop.selected = "";
                                                    root.dragging = card.modelData;
                                                }
                                                if (dragged)
                                                    root.at = mapToItem(root, event.x, event.y);
                                            }
                                            onReleased: {
                                                if (dragged)
                                                    root.drop();
                                                else
                                                    root.add(card.modelData);
                                                dragged = false;
                                            }
                                            onCanceled: {
                                                dragged = false;
                                                root.dragging = null;
                                            }
                                        }

                                        Rectangle {
                                            visible: card.activeFocus
                                            anchors.fill: parent
                                            anchors.margins: -3
                                            radius: card.radius + 3 // design: the ring follows the card's corner
                                            color: "transparent"
                                            border.width: 2
                                            border.color: Theme.accent
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Text {
                    anchors.centerIn: parent
                    width: parent.width
                    visible: root.groups.length === 0
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    text: root.looks.length === 0 ? "No widgets yet: the running modules offer none. Turn on a module that has some, like media or notes." : "No widgets match"
                    color: Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }

            // On desktop: each widget placed, its monitor, its settings.
            ListView {
                id: placedList

                anchors.fill: parent
                visible: root.tab === "placed"
                clip: true
                spacing: Theme.spaceTiny
                model: root.placed
                boundsBehavior: Flickable.StopAtBounds

                ScrollFade {
                    view: placedList
                }

                delegate: ListRow {
                    id: row

                    required property var modelData
                    readonly property bool running: (modelData.view ?? null) !== null
                    readonly property bool here: modelData.output === root.desktop.output

                    function settings(): void {
                        if (!running)
                            return;
                        if (here)
                            root.desktop.selected = modelData.id;
                        else
                            Daemon.command("widgets", "edit", ["on", modelData.output, modelData.id]);
                    }

                    width: placedList.width
                    icon: modelData.icon ?? "widgets"
                    leadingSize: 28
                    selected: root.desktop.selected === modelData.id
                    title: modelData.variantTitle ? `${modelData.title} · ${modelData.variantTitle}` : (modelData.title ?? modelData.widget)
                    // Where: "Top right, this screen", or the monitor's name.
                    subtitle: {
                        const corner = modelData.anchor.replace("-", " ");
                        const where = `${corner.charAt(0).toUpperCase()}${corner.slice(1)}, ${here ? "this screen" : modelData.output}`;
                        return running ? where : `${where}, waiting for the ${modelData.module} module`;
                    }
                    onClicked: settings()

                    trailing: [
                        ActionButton {
                            visible: row.running
                            icon: "edit"
                            tone: "ghost"
                            onClicked: row.settings()
                        },
                        ActionButton {
                            icon: "trash"
                            tone: "ghost"
                            onClicked: Daemon.command("widgets", "remove", [row.modelData.id])
                        }
                    ]
                }

                Column {
                    anchors.centerIn: parent
                    width: parent.width
                    visible: placedList.count === 0
                    spacing: Theme.spaceMedium

                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.query !== "" ? "No widgets placed match" : "Nothing on the desktop yet. Add a widget, and it shows up here."
                        color: Theme.muted
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }

                    ActionButton {
                        anchors.horizontalCenter: parent.horizontalCenter
                        visible: root.query === ""
                        text: "Add a widget"
                        icon: "plus"
                        onClicked: root.tab = "add"
                    }
                }
            }

            // Layouts: save this arrangement, switch, delete.
            Column {
                anchors.fill: parent
                visible: root.tab === "layouts"
                spacing: Theme.spaceMedium

                Row {
                    width: parent.width
                    spacing: Theme.spaceSmall

                    Rectangle {
                        width: parent.width - saveButton.width - parent.spacing
                        height: Theme.controlHeight
                        radius: Theme.radiusField
                        color: Theme.raised
                        border.width: name.activeFocus ? 1 : 0
                        border.color: Theme.accent

                        TextInput {
                            id: name

                            anchors.fill: parent
                            anchors.leftMargin: Theme.spaceMedium
                            anchors.rightMargin: Theme.spaceMedium
                            verticalAlignment: TextInput.AlignVCenter
                            activeFocusOnTab: true
                            maximumLength: 48
                            // A file name: no slashes.
                            validator: RegularExpressionValidator {
                                regularExpression: /[^\/\\]*/
                            }
                            color: Theme.foreground
                            selectionColor: Theme.accent
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            clip: true
                            onAccepted: root.saveLayout()

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: name.text === ""
                                text: "A name for this arrangement"
                                color: Theme.muted
                                font: name.font
                            }
                        }
                    }

                    ActionButton {
                        id: saveButton

                        height: Theme.controlHeight
                        enabled: name.text.trim() !== ""
                        text: root.layouts.some(layout => layout.name === name.text.trim()) ? "Replace" : "Save"
                        icon: "bookmark_add"
                        onClicked: root.saveLayout()
                    }
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.current !== "" ? `On the desktop: ${root.current}. Changes you make are kept in it.` : "This arrangement has no name. Switching keeps it as Unsaved."
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                Text {
                    visible: text !== ""
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.desktop.layout?.layoutError ?? ""
                    color: Theme.danger
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                ListView {
                    id: layoutList

                    width: parent.width
                    height: parent.height - y
                    clip: true
                    spacing: Theme.spaceTiny
                    model: root.layouts
                    boundsBehavior: Flickable.StopAtBounds

                    ScrollFade {
                        view: layoutList
                    }

                    delegate: ListRow {
                        id: saved

                        required property var modelData
                        readonly property bool current: modelData.name === root.current
                        readonly property bool confirming: root.confirming === modelData.name

                        width: layoutList.width
                        icon: current ? "check" : "bookmarks"
                        leadingSize: 28
                        selected: current
                        title: modelData.name
                        subtitle: `${modelData.count} ${modelData.count === 1 ? "widget" : "widgets"}${current ? ", on the desktop" : ""}`
                        onClicked: {
                            if (!current)
                                Daemon.command("widgets", "use-layout", [modelData.name]);
                        }

                        trailing: [
                            ActionButton {
                                visible: !saved.current && !saved.confirming
                                text: "Use"
                                onClicked: Daemon.command("widgets", "use-layout", [saved.modelData.name])
                            },
                            ActionButton {
                                visible: !saved.confirming
                                icon: "trash"
                                tone: "ghost"
                                onClicked: root.confirming = saved.modelData.name
                            },
                            ActionButton {
                                visible: saved.confirming
                                text: "Keep"
                                tone: "ghost"
                                onClicked: root.confirming = ""
                            },
                            ActionButton {
                                visible: saved.confirming
                                text: "Delete"
                                tone: "danger"
                                onClicked: {
                                    root.confirming = "";
                                    Daemon.command("widgets", "delete-layout", [saved.modelData.name]);
                                }
                            }
                        ]
                    }

                    Text {
                        anchors.top: parent.top
                        anchors.topMargin: Theme.spaceLarge
                        width: parent.width
                        visible: layoutList.count === 0
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: "No saved layouts. Name this arrangement and save it, to come back to it after trying another."
                        color: Theme.muted
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }
                }
            }
        }

        Column {
            id: footer

            x: Theme.padding
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.padding
            width: parent.width - Theme.padding * 2
            spacing: Theme.spaceSmall

            Text {
                visible: (root.desktop.layout?.error ?? null) !== null
                width: parent.width
                wrapMode: Text.Wrap
                text: root.desktop.layout?.error ?? ""
                color: Theme.danger
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            Row {
                width: parent.width
                spacing: Theme.spaceSmall

                // Copy the layout as widgets.toml; the arrow has the other formats.
                CopyButton {
                    id: copy

                    width: (parent.width - parent.spacing) / 2
                    module: "widgets"
                    onCopied: light.flash()
                }

                // The one accent: arranging ends here.
                ActionButton {
                    width: (parent.width - parent.spacing) / 2
                    text: "Done"
                    icon: "check"
                    tone: "accent"
                    onClicked: Daemon.command("widgets", "edit", ["off"])
                }
            }
        }
    }

    // The widget being dragged out, at its size on the grid, live.
    Rectangle {
        id: ghost

        readonly property real cell: root.desktop.cell
        readonly property var size: root.dragging ? (root.dragging.variant?.size ?? root.dragging.entry.size) : [0, 0]

        visible: root.dragging !== null
        width: size[0] * cell
        height: size[1] * cell
        x: Place.snap(root.at.x - width / 2, cell)
        y: Place.snap(root.at.y - height / 2, cell)
        radius: Theme.radiusSurface
        color: "transparent"
        border.width: 2
        border.color: Theme.accent

        Loader {
            anchors.fill: parent
            active: root.dragging !== null
            sourceComponent: Preview {
                entry: root.dragging.entry
                variant: root.dragging.variant
                cell: ghost.cell
                opacity: 0.85
            }
        }
    }

    // Placed where it was let go; the panel slid away as the drag began,
    // and comes back now.
    function drop(): void {
        const look = dragging;
        dragging = null;
        if (!look)
            return;
        const x = Math.max(0, Math.min(desktop.width - ghost.width, ghost.x));
        const y = Math.max(0, Math.min(desktop.height - ghost.height, ghost.y));
        const spot = Place.place(x, y, ghost.width, ghost.height, ghost.cell, desktop.width, desktop.height);
        Daemon.command("widgets", "add", [look.entry.module, look.key, desktop.output, spot.anchor, `${spot.x}`, `${spot.y}`]);
    }
}

import QtQuick
import qs.island

// The emoji picker: a search box, a tab per group, and a grid. The module
// publishes the whole table once, so typing filters here without asking.
// Arrows move the selection, Tab and Shift+Tab change the tab, Enter
// pastes the selected emoji into the window you were in, Shift+Enter
// copies it, Escape closes. A click pastes; a right click or Shift+click
// copies. The name of the emoji under the pointer shows at the bottom.
//
// The swatches by the search box set the skin tone of every emoji of a
// person or a hand. Holding one of those for a moment opens its tones;
// picking one pastes it, and that emoji keeps that tone from then on.
Item {
    id: root

    property var payload: ({})

    readonly property var table: Daemon.state("emoji")
    readonly property var groups: table?.groups ?? []
    readonly property var recent: table?.recent ?? []
    // The default skin tone, 0 for none and 1 to 5 from light to dark, and
    // the emoji with a tone of their own, by the emoji without one.
    readonly property int tone: table?.tone ?? 0
    readonly property var chosenTones: table?.tones ?? ({})
    // As the module's action takes them, in the same order.
    readonly property var toneNames: ["none", "light", "medium-light", "medium", "medium-dark", "dark"]
    // The swatches: the yellow of emoji without a tone, then the five.
    readonly property var toneColors: ["#ffcc4d", "#f7dece", "#f3d2a2", "#d5ab88", "#af7e57", "#7c533e"] // design: the colors of Unicode skin tones, not the theme
    // Each emoji as {glyph, name, group, words, subgroup, tones, index},
    // from the published [glyph, name, group, name words, subgroup words,
    // tones]. Tones is the five toned emoji, or null.
    readonly property var entries: (table?.emoji ?? []).map((entry, index) => ({
                "glyph": entry[0],
                "name": entry[1],
                "group": entry[2],
                "words": entry[3].split(" "),
                "subgroup": entry[4].split(" "),
                "tones": entry[5] ?? null,
                "index": index
            }))
    readonly property var groupWords: groups.map(group => group.words.split(" "))
    readonly property var byGlyph: {
        const map = {};
        for (const entry of entries)
            map[entry.glyph] = entry;
        return map;
    }

    readonly property int columns: 9
    readonly property int cell: 44
    readonly property int rows: 6

    // The tab: -1 for Recent, otherwise a group's index.
    property int tab: 0
    // The emoji under the pointer, or null.
    property var hovered: null
    // A tab's title while the pointer is on it.
    property string hoveredTab: ""
    // The emoji whose tones show in the popup after a long press, or null.
    property var choosing: null
    // Where the popup points, in this item's coordinates.
    property point choosingAt: Qt.point(0, 0)

    readonly property var query: input.text.toLowerCase().split(/\s+/).filter(word => word !== "")
    readonly property var shown: {
        if (query.length > 0)
            return search(query);
        if (tab < 0)
            return recent.map(glyph => byGlyph[glyph]).filter(entry => entry !== undefined);
        const group = groups[tab];
        return group ? entries.slice(group.start, group.start + group.count) : [];
    }
    readonly property var selected: shown[grid.currentIndex] ?? null

    implicitWidth: columns * cell + 24
    // The same height whatever the grid shows, so typing doesn't resize it.
    implicitHeight: 8 + 48 + 1 + 44 + rows * cell + 34 + 8

    onShownChanged: {
        hovered = null;
        choosing = null;
        grid.currentIndex = 0;
        grid.positionViewAtBeginning();
    }

    Component.onCompleted: {
        tab = recent.length > 0 ? -1 : 0;
        Qt.callLater(() => input.forceActiveFocus());
    }

    // The search, the same as the module's in search.rs: every word must
    // start a word of the name, the subgroup or the group, or be the emoji.
    // 2 is an exact word, 1 a prefix, 0 no match.
    function matchWord(query: string, word: string): int {
        if (word === query)
            return 2;
        if (word.startsWith(query))
            return 1;
        // "smile" finds "smiling".
        if (query.endsWith("e")) {
            const stem = query.slice(0, -1);
            if (word.startsWith(stem) && word.slice(stem.length).startsWith("ing"))
                return 1;
        }
        return 0;
    }

    function best(query: string, words: var): int {
        let found = 0;
        for (const word of words)
            found = Math.max(found, matchWord(query, word));
        return found;
    }

    // Higher is better; -1 when a word matches nothing.
    function score(entry: var, query: var): int {
        let total = 0;
        for (const word of query) {
            if (entry.glyph === word || entry.glyph.replace(/️+$/, "") === word || (entry.tones?.includes(word) ?? false)) {
                total += 6;
                continue;
            }
            let found = best(word, entry.words);
            if (found > 0) {
                total += found === 2 ? 6 : 4;
                continue;
            }
            found = best(word, entry.subgroup);
            if (found > 0) {
                total += found === 2 ? 3 : 2;
                continue;
            }
            found = best(word, groupWords[entry.group] ?? []);
            if (found > 0) {
                total += found === 2 ? 2 : 1;
                continue;
            }
            return -1;
        }
        if (query.some(word => matchWord(word, entry.words[0]) > 0))
            total += 3;
        if (entry.words.length === query.length && entry.words.every((word, index) => word === query[index]))
            total += 10;
        return total;
    }

    // Best first; then shorter names; then the table's order.
    function search(query: var): var {
        const found = [];
        for (const entry of entries) {
            let points = score(entry, query);
            if (points < 0)
                continue;
            if (recent.includes(entry.glyph))
                points += 2;
            found.push({
                "entry": entry,
                "points": points
            });
        }
        found.sort((a, b) => b.points - a.points || a.entry.words.length - b.entry.words.length || a.entry.index - b.entry.index);
        return found.map(hit => hit.entry);
    }

    function move(by: int): void {
        const next = grid.currentIndex + by;
        if (next < 0 || next >= shown.length)
            return;
        grid.currentIndex = next;
        grid.positionViewAtIndex(next, GridView.Contain);
    }

    function changeTab(by: int): void {
        const count = groups.length + 1;
        tab = (tab + 1 + by + count) % count - 1;
        input.text = "";
    }

    // The tone an emoji shows in: its own, or the default.
    function toneOf(entry: var): int {
        return root.chosenTones[entry.glyph] ?? root.tone;
    }

    // The emoji in its tone.
    function toned(entry: var): string {
        const tone = entry.tones ? toneOf(entry) : 0;
        return tone > 0 ? entry.tones[tone - 1] : entry.glyph;
    }

    // Pastes the emoji into the window you were in, or only copies it.
    function send(entry: var, copy: bool): void {
        if (!entry)
            return;
        Daemon.command("emoji", copy ? "copy" : "paste", [toned(entry)]);
        // Copying leaves the picker open: its edge says it's done.
        if (copy)
            light.flash();
    }

    // Opens the tones of an emoji over its tile.
    function choose(entry: var, tile: Item): void {
        if (!entry?.tones)
            return;
        choosingAt = tile.mapToItem(root, tile.width / 2, 0);
        choosing = entry;
    }

    // Gives the emoji being chosen a tone of its own, and pastes or copies
    // it in that tone.
    function pickTone(tone: int, copy: bool): void {
        const entry = choosing;
        choosing = null;
        Daemon.command("emoji", "tone", [toneNames[tone], entry.glyph]);
        const glyph = tone > 0 ? entry.tones[tone - 1] : entry.glyph;
        Daemon.command("emoji", copy ? "copy" : "paste", [glyph]);
        if (copy)
            light.flash();
    }

    EdgeLight {
        id: light

        radius: Theme.radiusSurface
    }

    Column {
        id: column

        anchors.fill: parent
        anchors.topMargin: Theme.spaceSmall
        anchors.bottomMargin: Theme.spaceSmall

        Item {
            width: parent.width
            height: 48

            Symbol {
                id: magnifier

                x: Theme.padding + 4
                anchors.verticalCenter: parent.verticalCenter
                name: "search"
                size: 18
                color: Theme.muted
            }

            TextInput {
                id: input

                anchors.left: magnifier.right
                anchors.leftMargin: Theme.spaceMedium
                anchors.right: swatches.left
                anchors.rightMargin: Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                focus: true
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                clip: true

                // The arrows move in the grid, not in the text.
                Keys.onUpPressed: root.move(-root.columns)
                Keys.onDownPressed: root.move(root.columns)
                Keys.onLeftPressed: root.move(-1)
                Keys.onRightPressed: root.move(1)
                Keys.onTabPressed: root.changeTab(1)
                Keys.onBacktabPressed: root.changeTab(-1)
                Keys.onReturnPressed: event => root.send(root.selected, (event.modifiers & Qt.ShiftModifier) !== 0)
                Keys.onEnterPressed: event => root.send(root.selected, (event.modifiers & Qt.ShiftModifier) !== 0)
                Keys.onEscapePressed: {
                    if (root.choosing)
                        root.choosing = null;
                    else
                        Daemon.event("dismiss");
                }

                Text {
                    visible: input.text === ""
                    text: "Search emoji…"
                    color: Theme.muted
                    font: input.font
                }
            }

            // The default skin tone.
            Row {
                id: swatches

                anchors.right: parent.right
                anchors.rightMargin: Theme.padding
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Repeater {
                    model: root.toneColors

                    Item {
                        id: swatch

                        required property string modelData
                        required property int index
                        readonly property bool current: root.tone === index

                        width: 22
                        height: 22

                        Rectangle {
                            anchors.fill: parent
                            radius: width / 2
                            color: "transparent"
                            border.width: swatch.current ? 2 : swatchArea.containsMouse ? 1 : 0
                            border.color: swatch.current ? Theme.accent : Theme.muted
                        }

                        Rectangle {
                            anchors.centerIn: parent
                            width: 14
                            height: 14
                            radius: height / 2
                            color: swatch.modelData
                        }

                        MouseArea {
                            id: swatchArea

                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onContainsMouseChanged: root.hoveredTab = containsMouse ? (swatch.index === 0 ? "No skin tone" : `Skin tone: ${root.toneNames[swatch.index]}`) : ""
                            onClicked: Daemon.command("emoji", "tone", [root.toneNames[swatch.index]])
                        }
                    }
                }
            }
        }

        Rectangle {
            width: parent.width
            height: 1
            color: Theme.raised
        }

        Row {
            id: tabs

            x: 12
            topPadding: 4
            height: 44

            Repeater {
                model: [
                    {
                        "title": "Recent",
                        "glyph": ""
                    }
                ].concat(root.groups)

                Rectangle {
                    id: tabItem

                    required property var modelData
                    required property int index
                    readonly property int value: index - 1
                    readonly property bool current: root.query.length === 0 && root.tab === value

                    width: (root.width - 24) / (root.groups.length + 1)
                    height: 36
                    radius: Theme.radiusControl
                    color: current ? Theme.raised : tabArea.containsMouse ? Theme.surface : "transparent"

                    Symbol {
                        anchors.centerIn: parent
                        visible: tabItem.index === 0
                        name: "clock"
                        size: 18
                        color: tabItem.current ? Theme.foreground : Theme.muted
                    }

                    Text {
                        anchors.centerIn: parent
                        visible: tabItem.index > 0
                        text: tabItem.modelData.glyph
                        opacity: tabItem.current ? 1 : 0.7
                        font.pixelSize: Theme.textHeadline
                        font.family: Theme.fontFamily
                    }

                    // The accent bar under the current tab.
                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        visible: tabItem.current
                        width: 14
                        height: 2
                        radius: 1
                        color: Theme.accent
                    }

                    MouseArea {
                        id: tabArea

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onContainsMouseChanged: root.hoveredTab = containsMouse ? tabItem.modelData.title : ""
                        onClicked: {
                            root.tab = tabItem.value;
                            input.text = "";
                        }
                    }
                }
            }
        }

        Item {
            x: 12
            width: root.columns * root.cell
            height: root.rows * root.cell

            Text {
                anchors.centerIn: parent
                visible: root.shown.length === 0
                text: root.query.length > 0 ? "No matches" : root.tab < 0 ? "The emoji you pick show here" : ""
                color: Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }

            GridView {
                id: grid

                anchors.fill: parent
                cellWidth: root.cell
                cellHeight: root.cell
                clip: true
                model: root.shown
                boundsBehavior: Flickable.StopAtBounds
                highlightFollowsCurrentItem: false

                ScrollFade {
                    view: grid
                }

                WheelScroll {
                    view: grid
                }

                delegate: Item {
                    id: tile

                    required property var modelData
                    required property int index
                    readonly property bool current: GridView.isCurrentItem

                    width: root.cell
                    height: root.cell

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 2 // design: a hairline gap between tiles
                        radius: Theme.radiusControl
                        color: tile.current ? Theme.raised : area.containsMouse ? Theme.surface : "transparent"
                        border.width: tile.current ? 1 : 0
                        border.color: Theme.accent
                    }

                    Text {
                        anchors.centerIn: parent
                        text: root.toned(tile.modelData)
                        font.pixelSize: 26 // design: an emoji sized to fill its 44 pixel tile
                        font.family: Theme.fontFamily
                    }

                    // A dot in the corner of emoji that come in tones.
                    Rectangle {
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: Theme.spaceSmall
                        visible: tile.modelData.tones !== null && (area.containsMouse || tile.current)
                        width: 4
                        height: 4
                        radius: 2
                        color: Theme.muted
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        pressAndHoldInterval: 400
                        onContainsMouseChanged: {
                            if (containsMouse)
                                root.hovered = tile.modelData;
                            else if (root.hovered === tile.modelData)
                                root.hovered = null;
                        }
                        // A long press opens the tones instead of pasting;
                        // MouseArea sends no click after it.
                        onPressAndHold: mouse => {
                            if (mouse.button === Qt.LeftButton && tile.modelData.tones)
                                root.choose(tile.modelData, tile);
                            else
                                mouse.accepted = false;
                        }
                        onClicked: mouse => root.send(tile.modelData, mouse.button === Qt.RightButton || (mouse.modifiers & Qt.ShiftModifier) !== 0)
                    }
                }
            }
        }

        // The name of the emoji under the pointer, or the selected one.
        Item {
            width: parent.width
            height: 34

            Text {
                anchors.left: parent.left
                anchors.leftMargin: Theme.padding + 2
                anchors.right: hint.left
                anchors.rightMargin: Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                text: {
                    if (root.hoveredTab !== "")
                        return root.hoveredTab;
                    const entry = root.choosing ?? root.hovered ?? root.selected;
                    return entry ? `${root.toned(entry)}  ${entry.name}` : "";
                }
                elide: Text.ElideRight
                textFormat: Text.PlainText
                color: Theme.foreground
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            Text {
                id: hint

                anchors.right: parent.right
                anchors.rightMargin: Theme.padding + 2
                anchors.verticalCenter: parent.verticalCenter
                text: {
                    if (root.choosing)
                        return "It keeps the tone you pick";
                    if (root.hovered?.tones)
                        return "Hold for skin tones";
                    return "Enter pastes · Shift+Enter copies";
                }
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }

    // While the tones show, a click anywhere else closes them.
    MouseArea {
        anchors.fill: parent
        visible: root.choosing !== null
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: root.choosing = null
    }

    // An emoji's tones after a long press, over its tile, or under it on
    // the top row. A click pastes one; a right click or Shift+click copies.
    Rectangle {
        id: tones

        readonly property bool below: root.choosingAt.y - height - 4 < 0

        visible: root.choosing !== null
        width: toneRow.width + 8
        height: root.cell + 8
        x: Math.max(4, Math.min(root.width - width - 4, root.choosingAt.x - width / 2))
        y: below ? root.choosingAt.y + root.cell + 4 : root.choosingAt.y - height - 4
        radius: Theme.radiusField
        color: Theme.raised
        border.width: 1
        border.color: Theme.border

        Row {
            id: toneRow

            anchors.centerIn: parent

            Repeater {
                model: root.toneNames

                Item {
                    id: choice

                    required property int index
                    readonly property bool current: root.choosing !== null && root.toneOf(root.choosing) === index

                    width: root.cell
                    height: root.cell

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 2 // design: a hairline gap between tiles
                        radius: Theme.radiusControl
                        color: choiceArea.containsMouse ? Theme.highlight : "transparent"
                        border.width: choice.current ? 1 : 0
                        border.color: Theme.accent
                    }

                    Text {
                        anchors.centerIn: parent
                        text: root.choosing ? (choice.index > 0 ? root.choosing.tones[choice.index - 1] : root.choosing.glyph) : ""
                        font.pixelSize: 26 // design: an emoji sized to fill its 44 pixel tile
                        font.family: Theme.fontFamily
                    }

                    MouseArea {
                        id: choiceArea

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        onClicked: mouse => root.pickTone(choice.index, mouse.button === Qt.RightButton || (mouse.modifiers & Qt.ShiftModifier) !== 0)
                    }
                }
            }
        }
    }
}

import QtQuick
import qs.island

// The emoji picker: a search box, a tab per group, and a grid. The module
// publishes the whole table once, so typing filters here without asking.
// Arrows move the selection, Tab and Shift+Tab change the tab, Enter
// pastes the selected emoji into the window you were in, Shift+Enter
// copies it, Escape closes. A click pastes; a right click or Shift+click
// copies. The name of the emoji under the pointer shows at the bottom.
Item {
    id: root

    property var payload: ({})

    readonly property var table: Daemon.state("emoji")
    readonly property var groups: table?.groups ?? []
    readonly property var recent: table?.recent ?? []
    // Each emoji as {glyph, name, group, words, subgroup, index}, from the
    // published [glyph, name, group, name words, subgroup words].
    readonly property var entries: (table?.emoji ?? []).map((entry, index) => ({
                "glyph": entry[0],
                "name": entry[1],
                "group": entry[2],
                "words": entry[3].split(" "),
                "subgroup": entry[4].split(" "),
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
            if (entry.glyph === word || entry.glyph.replace(/️+$/, "") === word) {
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
            found.push({ "entry": entry, "points": points });
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

    // Pastes the emoji into the window you were in, or only copies it.
    function send(entry: var, copy: bool): void {
        if (entry)
            Daemon.command("emoji", copy ? "copy" : "paste", [entry.glyph]);
    }

    Column {
        id: column

        anchors.fill: parent
        anchors.topMargin: 8
        anchors.bottomMargin: 8

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
                anchors.leftMargin: 12
                anchors.right: parent.right
                anchors.rightMargin: Theme.padding
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
                Keys.onEscapePressed: Daemon.event("dismiss")

                Text {
                    visible: input.text === ""
                    text: "Search emoji…"
                    color: Theme.muted
                    font: input.font
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
                model: [{ "title": "Recent", "glyph": "" }].concat(root.groups)

                Rectangle {
                    id: tabItem

                    required property var modelData
                    required property int index
                    readonly property int value: index - 1
                    readonly property bool current: root.query.length === 0 && root.tab === value

                    width: (root.width - 24) / (root.groups.length + 1)
                    height: 36
                    radius: Theme.radiusSmall
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
                        font.pixelSize: 20
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

                delegate: Item {
                    id: tile

                    required property var modelData
                    required property int index
                    readonly property bool current: GridView.isCurrentItem

                    width: root.cell
                    height: root.cell

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 2
                        radius: Theme.radiusSmall
                        color: tile.current ? Theme.raised : area.containsMouse ? Theme.surface : "transparent"
                        border.width: tile.current ? 1 : 0
                        border.color: Theme.accent
                    }

                    Text {
                        anchors.centerIn: parent
                        text: tile.modelData.glyph
                        font.pixelSize: 26
                        font.family: Theme.fontFamily
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        onContainsMouseChanged: {
                            if (containsMouse)
                                root.hovered = tile.modelData;
                            else if (root.hovered === tile.modelData)
                                root.hovered = null;
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
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                text: {
                    if (root.hoveredTab !== "")
                        return root.hoveredTab;
                    const entry = root.hovered ?? root.selected;
                    return entry ? `${entry.glyph}  ${entry.name}` : "";
                }
                elide: Text.ElideRight
                textFormat: Text.PlainText
                color: Theme.foreground
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }

            Text {
                id: hint

                anchors.right: parent.right
                anchors.rightMargin: Theme.padding + 2
                anchors.verticalCenter: parent.verticalCenter
                text: "Enter pastes · Shift+Enter copies"
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }
}

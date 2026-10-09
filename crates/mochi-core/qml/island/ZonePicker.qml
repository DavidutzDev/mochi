import QtQuick
import "Zones.js" as Zones

// Picks time zones from the system's list: a search field, then the zones
// that match, each as "Tokyo, Asia" with "UTC+9 · Japan" under it, chosen
// ones checked. Before anything is typed it lists the chosen zones, then
// `suggested`, then every zone west to east. Up and Down move the mark,
// Enter picks the marked zone, Escape says `closed`. A click or Enter says
// `picked` with the zone's name, and the owner adds or takes it off.
//
// The zones are what a module publishes as `timezones`, read only while a
// picker shows, since there are a few hundred: the clock's World tab, the
// settings' zone menu and the widget editor's clock settings use it. With
// `searchField` off, the owner's own field sets `query` and passes the keys
// on with move() and accept(), and only matches show.
Item {
    id: root

    // {value, label, detail} for each zone; null while they're read.
    property var zones: null
    // The zones picked now, by name, like ["Asia/Tokyo"].
    property var chosen: []
    // Shown after the chosen ones before anything is typed.
    property var suggested: []
    property var titles: ({
            "chosen": "Chosen",
            "suggested": "Suggested",
            "all": "Every zone"
        })
    // The most zones `chosen` may hold, 0 for no limit. Once it's full the
    // others dim and can't be picked; the owner says why.
    property int most: 0
    // Offers what's typed as itself when no zone has that name.
    property bool custom: false
    property bool searchField: true
    property string query: ""
    property string placeholder: "Search a city, a country or UTC+9"
    // The most rows without a search field, for a short list under a
    // field; 0 for all.
    property int limit: 0
    property color fieldColor: Theme.raised
    // The fades at the list's ends blend into this.
    property color color: Theme.background

    signal picked(string value)
    signal closed

    readonly property bool loading: zones === null || zones === undefined
    readonly property bool full: most > 0 && chosen.length >= most
    readonly property var rows: {
        if (loading)
            return [];
        const all = Zones.rows(zones, query, chosen, suggested, titles, custom);
        if (searchField)
            return all;
        // Under someone's field: matches only, and not too many.
        return query.trim() === "" ? [] : limit > 0 ? all.slice(0, limit) : all;
    }
    // The marked row, by index in `rows`, or -1; a heading is never
    // marked. A search marks its first match, for Enter; before one,
    // nothing is marked until Down, so Enter can't take a zone off.
    property int current: -1

    function open(row: var): bool {
        return row?.kind === "zone" && (!full || chosen.includes(row.value));
    }

    // The next zone row from `from`, `step` at a time, or -1.
    function next(from: int, step: int): int {
        for (let at = from; at >= 0 && at < rows.length; at += step) {
            if (rows[at].kind === "zone")
                return at;
        }
        return -1;
    }

    function move(step: int): void {
        const to = current < 0 ? next(0, 1) : next(current + step, step);
        if (to >= 0) {
            current = to;
            list.positionViewAtIndex(to, ListView.Contain);
        }
    }

    // Picks the marked zone; false when there's none to pick.
    function accept(): bool {
        const row = rows[current];
        if (!open(row))
            return false;
        picked(row.value);
        return true;
    }

    function focusSearch(): void {
        field.forceActiveFocus();
    }

    // Puts `text` in the search field, like a search asked for from outside.
    function type(text: string): void {
        field.text = text;
    }

    function clear(): void {
        type("");
    }

    // The search `rows` were last made for. The mark is set here rather
    // than when `query` changes, which comes before `rows` follows it.
    property string listed: ""

    onRowsChanged: {
        // A new search, or the zones came after it: its first match.
        if (query !== listed || current < 0) {
            if (query !== listed)
                list.positionViewAtBeginning();
            listed = query;
            current = query.trim() === "" ? -1 : next(0, 1);
            return;
        }
        // A pick moves rows about: the mark stays on a zone near where it
        // was.
        if (current >= rows.length || rows[current]?.kind !== "zone") {
            const back = next(Math.min(current, rows.length - 1), -1);
            current = back >= 0 ? back : next(0, 1);
        }
    }

    // Under a field the rows are all zones, matches with no headings.
    implicitHeight: searchField ? 320 : rows.length * Theme.rowHeight

    Rectangle {
        id: search

        visible: root.searchField
        width: parent.width
        height: root.searchField ? Theme.controlHeight + Theme.spaceTiny : 0
        radius: Theme.radiusControl
        color: root.fieldColor
        border.width: field.activeFocus ? 1 : 0
        border.color: Theme.accent

        Symbol {
            id: lens

            x: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            name: "search"
            size: 16
            color: Theme.muted
        }

        TextInput {
            id: field

            anchors.left: lens.right
            anchors.leftMargin: Theme.spaceSmall
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            clip: true
            selectByMouse: true
            color: Theme.foreground
            selectionColor: Theme.accent
            selectedTextColor: Theme.onAccent
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            onTextChanged: root.query = text
            Keys.onDownPressed: root.move(1)
            Keys.onUpPressed: root.move(-1)
            Keys.onReturnPressed: root.accept()
            Keys.onEnterPressed: root.accept()
            Keys.onEscapePressed: event => {
                root.closed();
                event.accepted = true;
            }

            Text {
                anchors.fill: parent
                verticalAlignment: Text.AlignVCenter
                visible: field.text === ""
                elide: Text.ElideRight
                text: root.placeholder
                color: Theme.muted
                font: field.font
            }
        }
    }

    ListView {
        id: list

        anchors.top: search.bottom
        anchors.topMargin: root.searchField ? Theme.spaceSmall : 0
        anchors.bottom: parent.bottom
        width: parent.width
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: root.rows
        reuseItems: true

        ScrollFade {
            view: list
            color: root.color
        }

        delegate: Item {
            id: row

            required property var modelData
            required property int index
            readonly property bool zone: modelData.kind === "zone"
            readonly property bool picked: zone && root.chosen.includes(modelData.value)
            readonly property bool open: root.open(modelData)
            readonly property bool marked: zone && index === root.current

            width: ListView.view.width
            height: zone ? Theme.rowHeight : Theme.controlHeight

            SectionLabel {
                visible: !row.zone
                x: Theme.spaceSmall
                anchors.bottom: parent.bottom
                anchors.bottomMargin: Theme.spaceTiny
                text: row.modelData.label
            }

            Rectangle {
                visible: row.zone
                anchors.fill: parent
                radius: Theme.radiusControl
                color: (row.marked || area.containsMouse) && row.open ? Theme.highlight : "transparent"
                opacity: row.open ? 1 : 0.4

                Column {
                    x: Theme.spaceMedium
                    width: parent.width - x - Theme.spaceHuge
                    anchors.verticalCenter: parent.verticalCenter

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: row.modelData.label ?? ""
                        color: Theme.foreground
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                        font.weight: row.picked ? Theme.weightTitle : Theme.weightBody
                    }

                    Text {
                        width: parent.width
                        visible: text !== ""
                        elide: Text.ElideRight
                        text: row.modelData.detail ?? ""
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }
                }

                Symbol {
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceMedium
                    anchors.verticalCenter: parent.verticalCenter
                    visible: row.picked
                    name: "check"
                    size: Theme.textBody
                    color: Theme.accent
                }

                MouseArea {
                    id: area

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: row.open ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: {
                        if (!row.open)
                            return;
                        root.current = row.index;
                        root.picked(row.modelData.value);
                    }
                }
            }
        }
    }

    // Reading, nothing at all, or nothing that matches.
    Text {
        visible: root.searchField && root.rows.length === 0
        anchors.top: search.bottom
        anchors.topMargin: Theme.spaceLarge
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: {
            if (root.loading)
                return "Reading the system's time zones…";
            if (root.zones.length === 0)
                return "This system lists no time zones: its zone database (tzdata) is missing. A zone's name, like Europe/Paris, can still be typed in the settings.";
            return `No time zone matches “${root.query.trim()}”. Try a city, a country or an offset like UTC+9.`;
        }
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }
}

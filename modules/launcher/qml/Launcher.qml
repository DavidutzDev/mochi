import QtQuick
import Quickshell
import qs.island

// The search box and the results. Each keystroke goes to the module, which
// answers with new results for that query, and again as slower providers
// answer. The selection lives here: arrows or Tab move it, Enter picks it,
// Shift+Enter does the other thing a result offers (runs a command in a
// terminal, copies an emoji, opens a file's folder), Escape closes, the pointer
// selects on hover and picks on click. The `compact` layout lists more
// results, one line each, with the description only on the selected one,
// at the end of its line.
Item {
    id: root

    property var payload: ({})
    // Results only count for what is typed now; older answers still on the
    // way are ignored.
    property var results: []
    readonly property bool compact: payload.layout === "compact"
    readonly property int rows: compact ? 9 : 7
    readonly property int rowHeight: compact ? 36 : 50
    readonly property int iconSize: compact ? 22 : 32
    // The headings among the rows that fit, so the list makes room for them.
    readonly property int headings: {
        if (!(payload.sections ?? false))
            return 0;
        const shown = results.slice(0, rows);
        return shown.filter((result, index) => index === 0 || result.section !== shown[index - 1].section).length;
    }

    implicitWidth: 560
    implicitHeight: column.implicitHeight + Theme.spaceSmall * 2

    onPayloadChanged: {
        if ((payload.query ?? "") !== input.text)
            return;
        // A slower provider's answer keeps the selection where it was, by
        // what it shows; the keys change with every query.
        const selected = results[list.currentIndex];
        results = payload.results ?? [];
        const same = selected ? results.findIndex(result => result.section === selected.section && result.title === selected.title) : -1;
        list.currentIndex = Math.max(same, 0);
    }

    Component.onCompleted: {
        // Opened with something typed already, like ":" for emoji.
        input.text = payload.query ?? "";
        input.cursorPosition = input.text.length;
        results = payload.results ?? [];
        Qt.callLater(() => input.forceActiveFocus());
    }

    function pick(index: int, terminal: bool): void {
        const result = results[index];
        if (result)
            Daemon.command("launcher", "activate", terminal ? [result.key, "true"] : [result.key]);
    }

    function move(by: int): void {
        if (results.length > 0)
            list.currentIndex = (list.currentIndex + by + results.length) % results.length;
    }

    EdgeLight {
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
                anchors.right: parent.right
                anchors.rightMargin: Theme.padding
                anchors.verticalCenter: parent.verticalCenter
                focus: true
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                clip: true

                onTextChanged: Daemon.command("launcher", "search", text ? [text] : [])

                Keys.onUpPressed: root.move(-1)
                Keys.onDownPressed: root.move(1)
                Keys.onTabPressed: root.move(1)
                Keys.onBacktabPressed: root.move(-1)
                Keys.onReturnPressed: event => root.pick(list.currentIndex, event.modifiers & Qt.ShiftModifier)
                Keys.onEnterPressed: event => root.pick(list.currentIndex, event.modifiers & Qt.ShiftModifier)
                Keys.onEscapePressed: Daemon.event("dismiss")

                Text {
                    visible: input.text === ""
                    text: "Search…"
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

        Text {
            visible: root.results.length === 0
            width: parent.width
            height: root.rowHeight
            leftPadding: Theme.padding + 4
            verticalAlignment: Text.AlignVCenter
            text: root.payload.searching ? "Searching…" : "No results"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        ListView {
            id: list

            visible: root.results.length > 0
            width: parent.width
            height: Math.min(root.results.length, root.rows) * root.rowHeight + root.headings * 26 + 8
            topMargin: Theme.spaceSmall
            clip: true
            model: root.results
            boundsBehavior: Flickable.StopAtBounds

            ScrollFade {
                view: list
            }

            WheelScroll {
                view: list
            }

            // Headings only when results come from more than one provider.
            section.property: root.payload.sections ? "section" : ""
            section.delegate: Item {
                required property string section

                width: list.width
                height: 26

                SectionLabel {
                    x: Theme.padding + 4
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: Theme.spaceTiny
                    text: parent.section
                }
            }
            delegate: ListRow {
                id: row

                required property var modelData
                required property int index

                x: Theme.spaceSmall
                width: list.width - Theme.spaceSmall * 2
                height: root.rowHeight
                flat: true
                marker: true
                selected: ListView.isCurrentItem
                leadingSize: root.iconSize
                title: modelData.title
                subtitle: root.compact ? "" : modelData.subtitle ?? ""
                onHoveredChanged: {
                    if (hovered)
                        list.currentIndex = index;
                }
                onClicked: root.pick(index, false)
                // A file drags out onto other apps.
                file: modelData.file ?? ""

                trailing: [
                    // Compact: the description at the end of the line, on
                    // the selected row only, given at most half the row.
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: root.compact && row.selected && text !== ""
                        width: Math.min(implicitWidth, row.width / 2)
                        text: row.modelData.subtitle ?? ""
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    },
                    // A file's folder, with it selected, as Shift+Enter.
                    IconButton {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: row.selected && row.file !== ""
                        icon: "folder_open"
                        size: 14
                        tone: "neutral"
                        onClicked: root.pick(row.index, true)
                    }
                ]

                leading: Item {
                    anchors.fill: parent

                    // A color's swatch instead of an icon.
                    Rectangle {
                        anchors.centerIn: parent
                        visible: row.modelData.color != null
                        width: root.iconSize - 6
                        height: root.iconSize - 6
                        radius: height / 2
                        color: row.modelData.color ?? "transparent"
                        border.width: 1
                        border.color: Theme.border
                    }

                    // A short text instead of an icon, like an emoji or "=".
                    Text {
                        anchors.centerIn: parent
                        visible: row.modelData.glyph != null && row.modelData.color == null
                        text: row.modelData.glyph ?? ""
                        color: Theme.foreground
                        font.pixelSize: root.compact ? Theme.textTitle : Theme.textHeadline
                        font.family: Theme.fontFamily
                    }

                    Image {
                        id: icon

                        anchors.fill: parent
                        visible: row.modelData.glyph == null && row.modelData.color == null
                        // Actions get a smaller icon, a step in.
                        anchors.margins: row.modelData.small ? (root.compact ? Theme.spaceTiny : Theme.spaceSmall) : 0
                        source: {
                            const name = row.modelData.icon ?? "";
                            if (name.startsWith("/"))
                                return `file://${name}`;
                            return name ? Quickshell.iconPath(name, true) : "";
                        }
                        sourceSize.width: 64
                        sourceSize.height: 64
                        fillMode: Image.PreserveAspectFit
                        // A theme icon loads here: Qt's icon themes break when read from
                        // the loading thread while this one reads them too. Files load
                        // in the background.
                        asynchronous: !String(source).startsWith("image://icon/")
                    }

                    // No such icon in the theme: a symbol by that name, like
                    // the settings' "palette", on a plain tile.
                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: root.compact ? 0 : Theme.spaceTiny
                        visible: row.modelData.glyph == null && row.modelData.color == null && icon.status !== Image.Ready
                        radius: Theme.radiusControl
                        color: Theme.raised

                        Symbol {
                            anchors.centerIn: parent
                            visible: (row.modelData.icon ?? "") !== "" && !row.modelData.icon.startsWith("/")
                            name: row.modelData.icon ?? ""
                            size: root.compact ? Theme.textBody : Theme.textTitle + Theme.spaceTiny
                            color: Theme.foreground
                        }
                    }
                }
            }
        }
    }
}

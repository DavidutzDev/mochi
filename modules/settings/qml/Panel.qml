import QtQuick
import QtQuick.Window
import qs.island

// The settings panel: the sections in a sidebar, grouped as Appearance,
// Shell, Modules and Plugins, and the options of the open one beside it.
// Every change applies at once. Typing searches every option, and
// "@modified" lists the ones changed here. Copy gives
// everything that isn't a default as the `settings` and `theme` of
// home-manager's programs.mochi, or as TOML.
//
// The lists are built from the paths of what they show, not from the
// options themselves: each change brings new values, and rebuilding the
// rows would drop a slider being dragged or a field being typed in.
Item {
    id: root

    property var payload: ({})
    readonly property var settings: Daemon.state("settings") ?? ({})
    readonly property var sections: settings.sections ?? []
    readonly property var enabled: settings.enabled ?? []

    // The open section's id, and an option to point at, by path.
    property string section: ""
    property string option: ""
    property string query: ""
    property bool editing: false

    readonly property var current: sections.find(entry => entry.id === section) ?? sections[0] ?? null
    // Bento's pages, which aren't options.
    readonly property bool bentoPage: current?.group === "bento"
    // The About page, which isn't options either.
    readonly property bool aboutPage: current?.id === "about"
    readonly property string bentoConsent: "config.bento.i_really_understand_that_bento_can_harm_and_contain_malicious_content"
    readonly property var error: settings.error ?? null

    implicitWidth: 960
    implicitHeight: Math.max(420, Math.min(640, Theme.surfaceHeight - Theme.margin, (Screen.height > 0 ? Screen.height : 1080) - 220))

    focus: true
    Keys.onEscapePressed: root.back()
    Keys.onPressed: event => {
        if (event.key === Qt.Key_F && event.modifiers & Qt.ControlModifier) {
            search.forceActiveFocus();
            event.accepted = true;
        }
    }

    onPayloadChanged: go(payload.section ?? "", payload.option ?? "")
    Component.onCompleted: {
        go(payload.section ?? "", payload.option ?? "");
        Qt.callLater(() => search.forceActiveFocus());
    }

    function go(id: string, path: string): void {
        if (id !== "")
            section = id;
        option = path;
        query = "";
        editing = false;
        closePopup();
        body.contentY = 0;
    }

    // Escape closes what's on top: a menu, the editor, the search, then
    // the panel.
    function back(): void {
        if (popupKind !== "")
            closePopup();
        else if (editing)
            editing = false;
        else if (query !== "")
            query = "";
        else
            Daemon.event("dismiss");
    }

    // Every option by path, for the rows to read their values from.
    readonly property var fields: {
        const map = {};
        for (const entry of sections)
            for (const field of entry.fields)
                map[field.path] = field;
        return map;
    }

    // Changed in the panel, over what the files say.
    function modified(field: var): bool {
        return field.kind !== "group" && field.changed === true;
    }

    function sectionModified(entry: var): bool {
        return entry.id !== "modules" && entry.fields.some(modified);
    }

    function sectionOf(id: string): var {
        return sections.find(entry => entry.id === id) ?? null;
    }

    // The heading a nested option sits under, like "CPU" for cpu.notice.
    function prefix(path: string): string {
        const parent = fields[path.slice(0, path.lastIndexOf("."))];
        return parent?.kind === "group" ? parent.title : "";
    }

    // Search.
    readonly property string needle: query.trim().toLowerCase()
    readonly property bool modifiedOnly: needle === "@modified"

    function hit(field: var, entry: var): bool {
        if (field.kind === "group")
            return false;
        if (modifiedOnly)
            return modified(field);
        const text = `${field.title} ${field.description} ${field.path} ${prefix(field.path)} ${entry.title}`.toLowerCase();
        return needle.split(/\s+/).every(word => text.includes(word));
    }

    // What the content shows: [{section, paths}], one block for the open
    // section, or one per section with a match while searching.
    readonly property string layoutKey: {
        if (needle === "") {
            const entry = current;
            if (!entry)
                return "[]";
            // The Modules and Bento pages draw their own content.
            const paths = entry.id === "modules" || entry.id === "about" || entry.group === "bento" ? [] : entry.fields.map(field => field.path);
            return JSON.stringify([
                {
                    "section": entry.id,
                    "paths": paths
                }
            ]);
        }
        const blocks = [];
        for (const entry of sections) {
            if (entry.id === "modules")
                continue;
            const titled = !modifiedOnly && entry.title.toLowerCase().includes(needle);
            const paths = entry.fields.filter(field => titled || hit(field, entry)).map(field => field.path);
            if (paths.length > 0)
                blocks.push({
                    "section": entry.id,
                    "paths": paths
                });
        }
        return JSON.stringify(blocks);
    }
    readonly property var layout: JSON.parse(layoutKey)

    // The sidebar: a heading per group, then its sections' ids. While
    // searching, only the sections with a match.
    readonly property string sidebarKey: {
        const groups = [["appearance", "Appearance"], ["shell", "Shell"], ["modules", "Modules"], ["bento", "Bento"], ["plugins", "Plugins"], ["about", "Mochi"]];
        const found = needle === "" ? null : layout.map(block => block.section);
        const rows = [];
        for (const [group, title] of groups) {
            const ids = sections.filter(entry => entry.group === group && (found === null || found.includes(entry.id))).map(entry => entry.id);
            if (ids.length === 0)
                continue;
            rows.push({
                "heading": title
            });
            for (const id of ids)
                rows.push({
                    "id": id
                });
        }
        return JSON.stringify(rows);
    }
    readonly property var sidebar: JSON.parse(sidebarKey)

    // Up and Down in the search box move through the sidebar.
    function step(by: int): void {
        const ids = sidebar.filter(row => row.id).map(row => row.id);
        if (ids.length === 0)
            return;
        const at = ids.indexOf(current?.id ?? "");
        const next = ids[(at + by + ids.length) % ids.length];
        if (needle === "")
            go(next, "");
        else
            section = next;
    }

    // Turning a module on or off.
    function toggleModule(id: string, on: bool): void {
        const list = enabled.filter(module => module !== id);
        if (on)
            list.push(id);
        Daemon.command("settings", "set", ["config.modules", JSON.stringify(list)]);
    }

    // The editor closes once the daemon took what it saved.
    property bool editorSeen: false
    readonly property var editorState: settings.editor ?? null
    onEditorStateChanged: {
        if (editorState !== null)
            editorSeen = true;
        else if (editing && editorSeen && error === null)
            editing = false;
    }

    function edit(): void {
        editorSeen = false;
        editing = true;
        Daemon.command("settings", "text", [current.path]);
    }

    // The menu of a choice, or the color picker, over everything.
    property string popupKind: ""
    property Item popupOwner: null
    property point popupAt: Qt.point(0, 0)

    function openPopup(kind: string, owner: Item, anchor: Item): void {
        popupOwner = owner;
        popupKind = kind;
        const below = anchor.mapToItem(root, 0, anchor.height + Theme.spaceTiny);
        popupAt = below;
    }

    function closePopup(): void {
        popupKind = "";
        popupOwner = null;
    }

    EdgeLight {
        radius: Theme.radiusSurface
    }

    // The sidebar.
    Item {
        id: side

        width: 232
        height: parent.height

        Rectangle {
            id: searchBox

            x: Theme.spaceMedium
            y: Theme.spaceMedium
            width: parent.width - Theme.spaceMedium * 2
            height: Theme.controlHeight + Theme.spaceTiny
            radius: height / 2
            color: Theme.surface
            border.width: search.activeFocus ? 1 : 0
            border.color: Theme.raised

            Symbol {
                id: magnifier

                x: Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                name: "search"
                size: Theme.textTitle
                color: Theme.muted
            }

            TextInput {
                id: search

                anchors.left: magnifier.right
                anchors.leftMargin: Theme.spaceSmall
                anchors.right: parent.right
                anchors.rightMargin: Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                clip: true
                text: root.query
                color: Theme.foreground
                selectionColor: Theme.accent
                selectedTextColor: Theme.onAccent
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                onTextChanged: root.query = text

                Keys.onUpPressed: root.step(-1)
                Keys.onDownPressed: root.step(1)
                Keys.onEscapePressed: root.back()
                Keys.onReturnPressed: {
                    if (root.layout.length > 0)
                        root.go(root.layout[0].section, root.layout[0].paths[0] ?? "");
                }

                Text {
                    visible: search.text === ""
                    text: "Search settings"
                    color: Theme.muted
                    font: search.font
                }
            }
        }

        ListView {
            id: list

            anchors.top: searchBox.bottom
            anchors.topMargin: Theme.spaceSmall
            anchors.bottom: undo.top
            anchors.bottomMargin: Theme.spaceSmall
            x: Theme.spaceSmall
            width: parent.width - Theme.spaceSmall * 2
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: root.sidebar

            ScrollFade {
                view: list
            }

            delegate: Item {
                id: row

                required property var modelData
                readonly property var entry: modelData.id ? root.sectionOf(modelData.id) : null
                readonly property bool selected: entry !== null && root.current?.id === entry.id
                // A module that isn't running.
                readonly property bool off: entry?.module !== undefined && entry?.enabled === false

                width: ListView.view.width
                height: modelData.heading ? Theme.textCaption + Theme.spaceMedium * 2 : Theme.controlHeight + Theme.spaceTiny

                Text {
                    visible: row.modelData.heading !== undefined
                    x: Theme.spaceSmall
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: Theme.spaceTiny
                    text: row.modelData.heading ?? ""
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                }

                Rectangle {
                    visible: row.entry !== null
                    anchors.fill: parent
                    radius: Theme.radiusField
                    color: row.selected ? Theme.raised : area.containsMouse ? Theme.surface : "transparent"

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.fast
                        }
                    }

                    Symbol {
                        id: icon

                        x: Theme.spaceSmall
                        anchors.verticalCenter: parent.verticalCenter
                        name: row.entry?.icon ?? ""
                        size: Theme.textTitle + 2
                        color: row.selected ? Theme.foreground : Theme.muted
                        opacity: row.off ? 0.5 : 1
                    }

                    Text {
                        anchors.left: icon.right
                        anchors.leftMargin: Theme.spaceSmall
                        anchors.right: dot.left
                        anchors.rightMargin: Theme.spaceSmall
                        anchors.verticalCenter: parent.verticalCenter
                        elide: Text.ElideRight
                        text: row.entry?.title ?? ""
                        color: row.off ? Theme.muted : Theme.foreground
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                        font.weight: row.selected ? Theme.weightLabel : Theme.weightBody
                    }

                    // Something in it was changed here.
                    Rectangle {
                        id: dot

                        anchors.right: parent.right
                        anchors.rightMargin: Theme.spaceMedium
                        anchors.verticalCenter: parent.verticalCenter
                        visible: row.entry !== null && root.sectionModified(row.entry)
                        width: 6
                        height: 6
                        radius: width / 2
                        color: Theme.accent
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.go(row.entry.id, "")
                    }
                }
            }
        }

        // Back to what the files say, with a second click to be sure.
        Button {
            id: undo

            property bool armed: false

            x: Theme.spaceMedium
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceMedium
            width: parent.width - Theme.spaceMedium * 2
            visible: root.settings.changes === true
            height: visible ? implicitHeight : 0
            text: armed ? "Click again to undo them all" : "Undo all changes"
            icon: "undo"
            tone: armed ? "danger" : "ghost"
            onClicked: {
                if (armed) {
                    armed = false;
                    Daemon.command("settings", "discard", []);
                } else {
                    armed = true;
                    disarm.restart();
                }
            }

            Timer {
                id: disarm

                interval: 3000
                onTriggered: undo.armed = false
            }
        }
    }

    Rectangle {
        id: rule

        anchors.left: side.right
        y: Theme.spaceMedium
        width: 1
        height: parent.height - Theme.spaceMedium * 2
        color: Theme.raised
    }

    // The open section, or what a search found.
    Item {
        id: content

        anchors.left: rule.right
        anchors.right: parent.right
        height: parent.height

        readonly property bool searching: root.needle !== ""

        // Over the options, for the copy menu.
        Item {
            id: header

            z: 2
            x: Theme.spaceLarge
            y: Theme.spaceMedium
            width: parent.width - Theme.spaceLarge * 2
            height: Theme.controlHeight + Theme.spaceTiny

            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceSmall

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: content.searching ? "search" : root.current?.icon ?? ""
                    size: Theme.textHeadline
                    color: Theme.foreground
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: content.searching ? (root.modifiedOnly ? "Changed here" : `Results for “${root.query.trim()}”`) : root.current?.title ?? ""
                    color: Theme.foreground
                    font.pixelSize: Theme.textHeadline
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                // A module's own switch.
                Switch {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: !content.searching && root.current?.module !== undefined
                    enabled: root.settings.fixed_modules !== true
                    checked: root.current?.enabled === true
                    onToggled: checked => root.toggleModule(root.current.module, checked)
                }

                // Whether Bento is on, on its pages.
                Switch {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: !content.searching && root.bentoPage
                    checked: root.fields[root.bentoConsent]?.value === true
                    onToggled: checked => Daemon.command("settings", "set", [root.bentoConsent, checked ? "true" : "false"])
                }
            }

            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceSmall

                Button {
                    visible: !content.searching && root.current?.module === "tour"
                    text: "Take the tour"
                    icon: "tour"
                    tone: "accent"
                    onClicked: Daemon.command("tour", "start", [])
                }

                Button {
                    visible: !content.searching && root.current !== null && !root.bentoPage && root.sectionModified(root.current)
                    text: "Reset"
                    icon: "restart_alt"
                    tone: "ghost"
                    onClicked: Daemon.command("settings", "reset", [root.current.path])
                }

                Button {
                    visible: !content.searching && root.current !== null && root.current.id !== "modules" && !root.bentoPage && !root.aboutPage
                    text: root.editing ? "Options" : "TOML"
                    icon: root.editing ? "tune" : "code"
                    tone: root.editing ? "neutral" : "ghost"
                    onClicked: root.editing ? root.editing = false : root.edit()
                }

                CopyButton {
                    visible: !root.bentoPage && !root.aboutPage
                    width: 120
                    module: "settings"
                    down: true
                    formats: [
                        {
                            "label": "Copy as Nix",
                            "format": "nix"
                        },
                        {
                            "label": "Copy as TOML",
                            "format": "toml"
                        }
                    ]
                }
            }
        }

        Text {
            id: about

            anchors.top: header.bottom
            anchors.topMargin: Theme.spaceSmall
            x: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2
            visible: text !== ""
            wrapMode: Text.Wrap
            text: {
                if (content.searching)
                    return root.layout.length === 0 ? "Nothing matches." : "";
                if (root.current?.module !== undefined && root.current?.enabled === false)
                    return `${root.current.description} It's off: its options apply once it's on.`.trim();
                return root.current?.description ?? "";
            }
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Item {
            id: main

            anchors.top: about.visible ? about.bottom : header.bottom
            anchors.topMargin: Theme.spaceMedium
            // Above the bar of what's being tried, which isn't a sibling to
            // anchor to.
            anchors.bottom: parent.bottom
            anchors.bottomMargin: tried.visible ? tried.height + Theme.spaceMedium * 2 : Theme.spaceMedium
            x: Theme.spaceSmall
            width: parent.width - Theme.spaceSmall * 2

            Loader {
                anchors.fill: parent
                anchors.leftMargin: Theme.spaceSmall
                anchors.rightMargin: Theme.spaceSmall
                active: root.editing && !content.searching && root.current !== null
                sourceComponent: Editor {
                    path: root.current.path
                    sent: root.editorState
                    error: root.error?.path === root.current.path ? root.error.message : ""
                    onClosed: root.editing = false
                }
            }

            Flickable {
                id: body

                anchors.fill: parent
                visible: !root.editing || content.searching
                contentWidth: width
                contentHeight: blocks.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                // Scrolls an option into view.
                function reveal(item: Item): void {
                    const top = item.mapToItem(blocks, 0, 0).y;
                    const bottom = top + item.height;
                    if (top < contentY || bottom > contentY + height)
                        contentY = Math.max(0, Math.min(top - Theme.spaceHuge, contentHeight - height));
                }

                ScrollFade {
                    view: body
                }

                Column {
                    id: blocks

                    width: body.width
                    spacing: Theme.spaceLarge

                    // Bento's pages.
                    Bento {
                        visible: !content.searching && root.bentoPage
                        width: parent.width
                        page: root.bentoPage ? root.current.id : ""
                    }

                    About {
                        visible: !content.searching && root.aboutPage
                        width: parent.width
                    }

                    // What a module shows on its own page above its
                    // options, offered as a "section" to the settings,
                    // like the Updates page's status and changelog.
                    Repeater {
                        model: !content.searching && root.current?.module ? Daemon.offered("settings", "section").filter(entry => entry.module === root.current.module) : []

                        Loader {
                            id: offered

                            required property var modelData

                            width: blocks.width
                            Component.onCompleted: setSource(`root:/modules/${modelData.module}/${modelData.view}.qml`, {
                                payload: Daemon.state(modelData.module) ?? {}
                            })

                            Binding {
                                target: offered.item
                                property: "payload"
                                value: Daemon.state(offered.modelData.module) ?? {}
                                when: offered.item !== null
                            }
                        }
                    }

                    // The Modules page: a switch per module.
                    Column {
                        visible: !content.searching && root.current?.id === "modules"
                        width: parent.width
                        spacing: 2

                        Text {
                            visible: root.settings.fixed_modules === true
                            width: parent.width
                            leftPadding: Theme.spaceMedium
                            wrapMode: Text.Wrap
                            text: "mochid runs with --modules, which decides what runs."
                            color: Theme.muted
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }

                        Text {
                            visible: root.error?.path === "config.modules"
                            width: parent.width
                            leftPadding: Theme.spaceMedium
                            wrapMode: Text.Wrap
                            text: root.error?.message ?? ""
                            color: Theme.danger
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }

                        Repeater {
                            model: root.current?.id === "modules" ? (root.settings.modules ?? []).map(module => module.id) : []

                            ListRow {
                                id: moduleRow

                                required property string modelData
                                readonly property var module: (root.settings.modules ?? []).find(entry => entry.id === modelData) ?? {}

                                width: blocks.width
                                height: Theme.rowHeight
                                flat: true
                                leadingSize: 26
                                icon: module.icon ?? ""
                                title: module.title ?? modelData
                                subtitle: module.plugin ? "Plugin" : ""
                                onClicked: root.go(modelData, "")

                                trailing: Switch {
                                    anchors.verticalCenter: parent.verticalCenter
                                    enabled: root.settings.fixed_modules !== true
                                    checked: moduleRow.module.enabled === true
                                    onToggled: checked => root.toggleModule(moduleRow.modelData, checked)
                                }
                            }
                        }
                    }

                    Text {
                        visible: !content.searching && root.current !== null && root.current.id !== "modules" && root.current.fields.length === 0 && !root.bentoPage && !root.aboutPage
                        width: parent.width
                        leftPadding: Theme.spaceMedium
                        text: "Nothing to set here."
                        color: Theme.muted
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }

                    Repeater {
                        model: root.layout

                        Column {
                            id: block

                            required property var modelData
                            readonly property var entry: root.sectionOf(modelData.section)

                            width: blocks.width
                            spacing: 2

                            // While searching, which section the options are in.
                            Item {
                                visible: content.searching
                                width: parent.width
                                height: visible ? Theme.controlHeight : 0

                                Row {
                                    x: Theme.spaceMedium
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: Theme.spaceSmall

                                    Symbol {
                                        anchors.verticalCenter: parent.verticalCenter
                                        name: block.entry?.icon ?? ""
                                        size: Theme.textBody
                                        color: Theme.muted
                                    }

                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: block.entry?.title ?? ""
                                        color: Theme.muted
                                        font.pixelSize: Theme.textCaption
                                        font.family: Theme.fontFamily
                                        font.weight: Theme.weightLabel
                                    }
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: root.go(block.modelData.section, "")
                                }
                            }

                            Repeater {
                                model: block.modelData.paths

                                Option {
                                    id: row

                                    required property string modelData

                                    width: block.width
                                    field: root.fields[modelData] ?? ({
                                            "path": modelData,
                                            "kind": "group",
                                            "title": ""
                                        })
                                    prefix: content.searching ? root.prefix(modelData) : ""
                                    error: root.error?.path === modelData ? root.error.message : ""
                                    highlighted: root.option === modelData
                                    onPopup: (kind, anchor) => root.openPopup(kind, row, anchor)
                                    onEditToml: root.edit()
                                    onHighlightedChanged: {
                                        if (highlighted)
                                            Qt.callLater(() => body.reveal(row));
                                    }
                                    Component.onCompleted: {
                                        if (highlighted)
                                            Qt.callLater(() => body.reveal(row));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // What something else is trying, like `mochi ipc settings preview`:
    // kept as changes, or dropped. A reload drops it too. The panel's own
    // changes apply and are kept at once.
    Rectangle {
        id: tried

        visible: root.settings.previewing === true
        anchors.left: rule.right
        anchors.leftMargin: Theme.spaceLarge
        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceLarge
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.spaceMedium
        height: Theme.controlHeight + Theme.spaceSmall * 2
        radius: Theme.radiusField
        color: Theme.surface

        Row {
            x: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: "visibility"
                size: Theme.textTitle
                color: Theme.accent
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Trying changes. They apply, but aren't kept."
                color: Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceSmall

            Button {
                text: "Drop"
                tone: "ghost"
                onClicked: Daemon.command("settings", "drop", [])
            }

            Button {
                text: "Keep"
                icon: "check"
                tone: "accent"
                onClicked: Daemon.command("settings", "keep", [])
            }
        }
    }

    // The pointed-at option stays lit for a moment.
    Timer {
        running: root.option !== ""
        interval: 1800
        onTriggered: root.option = ""
    }

    // A click beside a menu closes it.
    MouseArea {
        anchors.fill: parent
        visible: root.popupKind !== ""
        z: 9
        onClicked: root.closePopup()
    }

    Rectangle {
        id: choices

        // What's typed in the search, which also offers itself as a value.
        property string query: ""
        readonly property var owner: root.popupKind === "choice" ? root.popupOwner : null
        readonly property var all: owner ? owner.choices() : []
        readonly property var shown: {
            const words = query.trim().toLowerCase();
            if (words === "")
                return all;
            return all.filter(choice => `${choice.label} ${choice.value} ${choice.detail ?? ""}`.toLowerCase().includes(words));
        }
        // A typed value that isn't in the menu, offered as itself.
        readonly property bool typed: (owner?.custom ?? false) && query.trim() !== "" && !all.some(choice => choice.value === query.trim())
        readonly property bool searching: owner?.searchable ?? false

        visible: root.popupKind === "choice"
        onVisibleChanged: {
            query = "";
            menuSearch.clear();
            if (visible && searching)
                Qt.callLater(() => menuSearch.forceActiveFocus());
        }
        z: 10
        x: Math.min(root.popupAt.x, root.width - width - Theme.spaceMedium)
        y: Math.min(root.popupAt.y, root.height - height - Theme.spaceMedium)
        width: searching ? 300 : 220
        height: Math.min(menu.implicitHeight + (searching ? menuSearch.height + Theme.spaceTiny : 0) + Theme.spaceTiny * 2, 360)
        radius: Theme.radiusField
        color: Theme.raised

        Entry {
            id: menuSearch

            visible: choices.searching
            x: Theme.spaceTiny
            y: Theme.spaceTiny
            width: parent.width - Theme.spaceTiny * 2
            placeholder: choices.owner?.custom ? "Search, or type one…" : "Search…"
            live: true
            onEdited: text => choices.query = text
            onAccepted: text => {
                const first = choices.typed ? text.trim() : choices.shown[0]?.value;
                if (first === undefined)
                    return;
                choices.owner.pick(first);
                // A list picks several, maybe from the same search: the
                // menu and the search stay.
                if (!choices.owner.multiple)
                    root.closePopup();
            }
        }

        Flickable {
            anchors.fill: parent
            anchors.margins: Theme.spaceTiny
            anchors.topMargin: choices.searching ? menuSearch.height + Theme.spaceTiny * 2 : Theme.spaceTiny
            contentHeight: menu.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds

            Column {
                id: menu

                width: parent.width

                Repeater {
                    // What's typed first, when it's offered as itself.
                    model: (choices.typed ? [
                            {
                                "value": choices.query.trim(),
                                "label": `Use “${choices.query.trim()}”`,
                                "icon": "add"
                            }
                        ] : []).concat(choices.shown)

                    Rectangle {
                        id: item

                        required property var modelData
                        readonly property bool picked: choices.owner?.isPicked(modelData.value) ?? false

                        width: menu.width
                        height: (modelData.detail ?? "") !== "" ? 44 : Theme.controlHeight
                        radius: Theme.radiusControl
                        color: itemArea.containsMouse ? Theme.highlight : "transparent"

                        Row {
                            x: Theme.spaceMedium
                            width: parent.width - x - Theme.spaceHuge
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: Theme.spaceSmall

                            // A preset's colors, overlapping a little.
                            Row {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: (item.modelData.colors ?? []).length > 0
                                spacing: -4

                                Repeater {
                                    model: item.modelData.colors ?? []

                                    Rectangle {
                                        required property string modelData

                                        width: 14
                                        height: 14
                                        radius: width / 2
                                        color: modelData
                                        border.width: 1
                                        border.color: Theme.highlight
                                    }
                                }
                            }

                            Symbol {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: (item.modelData.icon ?? "") !== ""
                                name: item.modelData.icon ?? ""
                                size: 18
                                color: Theme.muted
                            }

                            Column {
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - x

                                Text {
                                    width: parent.width
                                    text: item.modelData.label
                                    elide: Text.ElideRight
                                    color: Theme.foreground
                                    font.pixelSize: Theme.textBody
                                    font.family: Theme.fontFamily
                                    font.weight: item.picked ? Theme.weightTitle : Theme.weightBody
                                }

                                Text {
                                    width: parent.width
                                    visible: text !== ""
                                    text: item.modelData.detail ?? ""
                                    elide: Text.ElideRight
                                    color: Theme.muted
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                }
                            }
                        }

                        Symbol {
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.spaceSmall
                            anchors.verticalCenter: parent.verticalCenter
                            visible: item.picked
                            name: "check"
                            size: Theme.textBody
                            color: Theme.accent
                        }

                        MouseArea {
                            id: itemArea

                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                choices.owner.pick(item.modelData.value);
                                // A list picks several, maybe from the same
                                // search: the menu and the search stay.
                                if (!choices.owner.multiple)
                                    root.closePopup();
                            }
                        }
                    }
                }

                Text {
                    visible: choices.shown.length === 0 && !choices.typed
                    width: menu.width
                    height: Theme.controlHeight
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignHCenter
                    text: choices.all.length === 0 ? "Nothing to pick from right now" : "No match"
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }
            }
        }
    }

    // Time zones: the island's ZonePicker, every zone the system has with
    // a search, like the clock's World tab. One picked for a list, like the
    // clock's zones, is added or taken off and the picker stays.
    Rectangle {
        id: zoneMenu

        readonly property var owner: root.popupKind === "zones" ? root.popupOwner : null

        visible: owner !== null
        onVisibleChanged: {
            zonePicker.clear();
            if (visible)
                Qt.callLater(() => zonePicker.focusSearch());
        }
        z: 10
        x: Math.min(root.popupAt.x, root.width - width - Theme.spaceMedium)
        y: Math.min(root.popupAt.y, root.height - height - Theme.spaceMedium)
        width: 320
        height: 380
        radius: Theme.radiusField
        color: Theme.raised

        // Clicks inside don't reach the panel, which closes it.
        MouseArea {
            anchors.fill: parent
        }

        ZonePicker {
            id: zonePicker

            anchors.fill: parent
            anchors.margins: Theme.spaceTiny
            zones: Daemon.state("settings")?.timezones ?? null
            chosen: {
                const owner = zoneMenu.owner;
                if (!owner)
                    return [];
                return owner.multiple ? (owner.value ?? []).map(String) : [String(owner.value ?? "")];
            }
            custom: true
            color: Theme.raised
            fieldColor: Theme.highlight
            titles: ({
                    "chosen": "Chosen",
                    "suggested": "",
                    "all": "Every zone, west to east"
                })
            onPicked: value => {
                const owner = zoneMenu.owner;
                owner.pick(value);
                if (!owner.multiple)
                    root.closePopup();
            }
            onClosed: root.closePopup()
        }
    }

    // The fonts installed, each drawn in itself, with a search.
    Rectangle {
        id: fonts

        property string query: ""
        readonly property var found: root.popupKind === "font" ? Fonts.search(query) : []

        visible: root.popupKind === "font"
        z: 10
        x: Math.min(root.popupAt.x, root.width - width - Theme.spaceMedium)
        y: Math.min(root.popupAt.y, root.height - height - Theme.spaceMedium)
        width: 300
        height: 340
        radius: Theme.radiusField
        color: Theme.raised

        onVisibleChanged: {
            query = "";
            if (visible)
                Qt.callLater(() => fontSearch.forceActiveFocus());
        }

        // Clicks inside don't reach the panel, which closes it.
        MouseArea {
            anchors.fill: parent
        }

        Entry {
            id: fontSearch

            x: Theme.spaceTiny
            y: Theme.spaceTiny
            width: parent.width - Theme.spaceTiny * 2
            placeholder: `Search ${Fonts.families.length} fonts`
            color: Theme.highlight
            live: true
            onEdited: text => fonts.query = text
            onAccepted: text => {
                if (fonts.found.length > 0)
                    fonts.pick(fonts.found[0]);
            }
        }

        function pick(family: string): void {
            root.popupOwner?.sendNow(family);
            root.closePopup();
        }

        ListView {
            id: fontList

            anchors.top: fontSearch.bottom
            anchors.topMargin: Theme.spaceTiny
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceTiny
            x: Theme.spaceTiny
            width: parent.width - Theme.spaceTiny * 2
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            // The default first, while nothing is typed.
            model: fonts.query === "" ? [""].concat(fonts.found) : fonts.found
            reuseItems: true

            ScrollFade {
                view: fontList
                color: Theme.raised
            }

            delegate: Rectangle {
                id: fontRow

                required property string modelData
                readonly property bool picked: (root.popupOwner?.value ?? "") === modelData

                width: ListView.view.width
                height: Theme.controlHeight
                radius: Theme.radiusControl
                color: fontRowArea.containsMouse ? Theme.highlight : "transparent"

                Text {
                    x: Theme.spaceMedium
                    width: parent.width - x - Theme.spaceHuge
                    anchors.verticalCenter: parent.verticalCenter
                    elide: Text.ElideRight
                    text: fontRow.modelData !== "" ? fontRow.modelData : "Default"
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: fontRow.modelData !== "" ? fontRow.modelData : Theme.fontFamily
                }

                Symbol {
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter
                    visible: fontRow.picked
                    name: "check"
                    size: Theme.textBody
                    color: Theme.accent
                }

                MouseArea {
                    id: fontRowArea

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: fonts.pick(fontRow.modelData)
                }
            }
        }

        Text {
            anchors.centerIn: fontList
            visible: fonts.found.length === 0
            text: "No font by that name"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }

    Loader {
        active: root.popupKind === "color"
        z: 10
        x: Math.min(root.popupAt.x, root.width - (item?.width ?? 0) - Theme.spaceMedium)
        y: root.popupAt.y + (item?.height ?? 0) > root.height - Theme.spaceMedium ? Math.max(Theme.spaceMedium, root.popupAt.y - (item?.height ?? 0) - Theme.controlHeight - Theme.spaceSmall) : root.popupAt.y

        sourceComponent: ColorPopover {
            value: root.popupOwner?.value ?? "#000000" // design: a value to edit, not a color to draw
            onPicked: value => root.popupOwner?.sendSoon(value)
        }
    }
}

import QtQuick
import qs.island

// Bento's pages in the settings: Discover, the registry's packages with a
// page each; Installed, what Bento installed, with newer releases and what
// the registry withdrew; Share, making a bento of this setup. Installing
// first shows what it will do, as the terminal does, and runs only on
// Install. The daemon does the work with `mochid bento`, and its state
// says how it goes.
Column {
    id: root

    required property string page

    readonly property var bento: Daemon.state("settings")?.bento ?? ({})
    readonly property var catalog: bento.catalog ?? ({})
    readonly property var packages: catalog.packages ?? []
    readonly property var installed: catalog.installed ?? []
    readonly property var plan: bento.plan ?? null
    readonly property var job: bento.job ?? null
    readonly property bool busy: job?.running === true
    readonly property bool planning: (bento.planning ?? null) !== null

    // Discover's filters, and the package whose page is open.
    property string query: ""
    property string kind: "all"
    property string selected: ""
    readonly property var chosen: packages.find(entry => entry.id === selected) ?? null

    spacing: Theme.spaceMedium

    function load(refresh: bool): void {
        Daemon.command("settings", "bento-catalog", refresh ? ["refresh"] : []);
    }

    // The registry is read when a page shows, not when the panel opens.
    function arrive(): void {
        selected = "";
        if (page === "bento-installed" || (page === "bento" && catalog.packages === undefined && bento.loading !== true))
            load(false);
    }

    Component.onCompleted: arrive()
    onPageChanged: arrive()

    function shown(entry: var): bool {
        const tags = entry.tags ?? [];
        const fits = kind === "all" || (kind === "widget" ? tags.includes("widget") : entry.kind === kind);
        const words = query.trim().toLowerCase().split(/\s+/).filter(word => word !== "");
        const text = `${entry.id} ${entry.name} ${entry.description} ${tags.join(" ")}`.toLowerCase();
        return fits && words.every(word => text.includes(word));
    }

    function kindIcon(kind: string): string {
        return kind === "plugin" ? "extension" : kind === "theme" ? "palette" : "bento";
    }

    function kindName(kind: string): string {
        return kind === "plugin" ? "Plugin" : kind === "theme" ? "Theme" : "Bento";
    }

    // Small parts.

    component Note: Text {
        width: root.width
        leftPadding: Theme.spaceMedium
        rightPadding: Theme.spaceMedium
        wrapMode: Text.Wrap
        color: Theme.muted
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
    }

    component Field: Rectangle {
        id: field

        property alias text: input.text
        property string hint: ""
        signal accepted

        height: Theme.controlHeight
        radius: Theme.radiusField
        color: Theme.surface
        border.width: input.activeFocus ? 1 : 0
        border.color: Theme.raised

        TextInput {
            id: input

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: Theme.spaceMedium
            anchors.rightMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            clip: true
            color: Theme.foreground
            selectionColor: Theme.accent
            selectedTextColor: Theme.onAccent
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            Keys.onReturnPressed: field.accepted()
            Keys.onEscapePressed: event => {
                if (text !== "")
                    text = "";
                else
                    event.accepted = false;
            }

            Text {
                visible: input.text === ""
                text: field.hint
                color: Theme.muted
                font: input.font
            }
        }
    }

    // One fact about what installing does: an icon and a line.
    component Fact: Row {
        property string icon: ""
        property string text: ""
        property bool warn: false

        x: Theme.spaceMedium
        width: root.width - Theme.spaceMedium * 2
        spacing: Theme.spaceSmall

        Symbol {
            name: parent.icon
            size: Theme.textBody
            color: parent.warn ? Theme.danger : Theme.muted
        }

        Text {
            width: parent.width - Theme.textBody - Theme.spaceSmall
            text: parent.text
            wrapMode: Text.Wrap
            color: parent.warn ? Theme.danger : Theme.foreground
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }

    // What a plugin will run and read, from its plan.
    component PluginFacts: Column {
        property var facts: ({})

        width: root.width
        spacing: Theme.spaceTiny

        Fact {
            visible: (parent.facts.runs ?? null) !== null
            icon: "build"
            text: `Builds with ${parent.facts.runs}`
        }
        Fact {
            visible: parent.facts.downloads === true
            icon: "download"
            text: "Downloads its release, built already"
        }
        Fact {
            icon: "play_arrow"
            text: (parent.facts.starts ?? null) !== null ? `Starts ${parent.facts.starts} with mochid, as you` : "Views only, no program of its own"
        }
        Fact {
            visible: (parent.facts.needs ?? []).length > 0
            icon: "terminal"
            warn: (parent.facts.missing ?? []).length > 0
            text: `Runs ${(parent.facts.needs ?? []).join(", ")}` + ((parent.facts.missing ?? []).length > 0 ? `, and ${(parent.facts.missing ?? []).join(", ")} isn't installed` : "")
        }
        Fact {
            visible: (parent.facts.reads ?? []).length > 0
            icon: "visibility"
            text: `Reads the state of ${(parent.facts.reads ?? []).join(", ")}`
        }
        Fact {
            visible: (parent.facts.replaces ?? []).length > 0
            icon: "swap_horiz"
            text: `Replaces ${(parent.facts.replaces ?? []).join(", ")}`
        }
        Fact {
            visible: (parent.facts.actions ?? []).length > 0
            icon: "bolt"
            text: `Actions ${(parent.facts.actions ?? []).join(", ")}`
        }
    }

    // A theme's colors, dark and light.
    component Swatches: Row {
        property var colors: ({})

        spacing: -4

        Repeater {
            model: ["background", "surface", "raised", "accent", "foreground", "muted"]

            Rectangle {
                required property string modelData

                width: 18
                height: 18
                radius: width / 2
                color: parent.colors?.[modelData] ?? "transparent"
                border.width: 1
                border.color: Theme.highlight
            }
        }
    }

    // How the last install, removal, update or try went.
    Rectangle {
        visible: root.job !== null && root.job.action !== "try" && root.page !== "bento-share"
        width: root.width
        height: jobRow.implicitHeight + Theme.spaceSmall * 2
        radius: Theme.radiusField
        color: Theme.surface

        Row {
            id: jobRow

            x: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - Theme.spaceMedium * 2 - dismiss.width
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: root.busy ? "progress_activity" : (root.job?.error ?? null) !== null ? "error" : "check_circle"
                size: Theme.textTitle
                color: (root.job?.error ?? null) !== null ? Theme.danger : Theme.accent

                RotationAnimation on rotation {
                    running: root.busy
                    loops: Animation.Infinite
                    from: 0
                    to: 360
                    duration: Theme.duration(1000)
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - Theme.textTitle - Theme.spaceSmall
                wrapMode: Text.Wrap
                text: {
                    const job = root.job ?? {};
                    const what = job.target || "everything";
                    const doing = {
                        "add": ["Installing", "Installed"],
                        "remove": ["Removing", "Removed"],
                        "update": ["Updating", "Updated"]
                    }[job.action] ?? ["Working on", "Done with"];
                    if (job.running)
                        return `${doing[0]} ${what}…`;
                    if (job.error)
                        return `${what}: ${job.error}`;
                    return `${doing[1]} ${what}.`;
                }
                color: Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }
        }

        IconButton {
            id: dismiss

            visible: !root.busy
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceTiny
            anchors.verticalCenter: parent.verticalCenter
            icon: "close"
            onClicked: Daemon.command("settings", "bento-forget", [])
        }
    }

    // What installing something will do, before it does.
    Column {
        id: planned

        visible: root.page === "bento" && root.planning
        width: root.width
        spacing: Theme.spaceSmall

        readonly property var plan: root.plan ?? {}
        readonly property var details: plan.bento ?? null
        readonly property bool ready: root.plan !== null && plan.problem === undefined
        readonly property bool blocked: (details?.problem ?? null) !== null || (details?.plugins ?? []).some(plugin => plugin.problem !== undefined)

        Note {
            visible: root.plan === null
            text: `Looking at ${root.bento.planning ?? ""}…`
        }

        Note {
            visible: root.plan !== null && planned.plan.problem !== undefined
            color: Theme.danger
            text: planned.plan.problem ?? ""
        }

        Column {
            visible: planned.ready
            width: root.width
            spacing: Theme.spaceSmall

            Row {
                x: Theme.spaceMedium
                spacing: Theme.spaceSmall

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: root.kindIcon(planned.plan.kind ?? "")
                    size: Theme.textHeadline
                    color: Theme.accent
                }

                Column {
                    anchors.verticalCenter: parent.verticalCenter

                    Text {
                        text: `${planned.plan.name ?? ""} ${planned.plan.version ?? ""}`
                        color: Theme.foreground
                        font.pixelSize: Theme.textTitle
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightTitle
                    }

                    Text {
                        text: `${root.kindName(planned.plan.kind ?? "")} ${planned.plan.id ?? ""}` + ((planned.plan.authors ?? []).length > 0 ? `, by ${planned.plan.authors.join(", ")}` : "")
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }
                }
            }

            Note {
                visible: (planned.plan.description ?? "") !== ""
                color: Theme.foreground
                text: planned.plan.description ?? ""
            }

            Fact {
                icon: "link"
                text: `From ${planned.plan.source ?? ""}` + (planned.plan.at ? ` at ${planned.plan.at.slice(0, 10)}` : "")
            }

            PluginFacts {
                visible: planned.plan.kind === "plugin"
                facts: planned.plan.plugin ?? {}
            }

            Row {
                visible: planned.plan.kind === "theme"
                x: Theme.spaceMedium
                spacing: Theme.spaceLarge

                Swatches {
                    colors: planned.plan.theme?.dark ?? {}
                }
                Swatches {
                    colors: planned.plan.theme?.light ?? {}
                }
            }

            Fact {
                visible: (planned.plan.theme?.sets ?? []).length > 0
                icon: "text_fields"
                text: "Sets its own " + (planned.plan.theme?.sets ?? []).map(section => ({
                            "text": "fonts",
                            "layout": "island shape and spacing",
                            "motion": "motion"
                        })[section] ?? section).join(", ") + ", under what your theme.toml sets"
            }

            // A bento: what it changes, then each plugin it needs.
            Column {
                visible: planned.details !== null
                width: root.width
                spacing: Theme.spaceTiny

                Fact {
                    icon: "tune"
                    text: `${planned.details?.settings ?? 0} settings and ${planned.details?.theme_settings ?? 0} of the theme go over yours`
                }
                Fact {
                    visible: (planned.details?.themes ?? []).length > 0
                    icon: "palette"
                    text: ((planned.details?.themes ?? []).length === 1 ? "Brings the theme " : "Brings the themes ") + (planned.details?.themes ?? []).join(", ")
                }
                Fact {
                    visible: (planned.details?.widgets ?? 0) > 0
                    icon: "widgets"
                    warn: (planned.details?.screens ?? 0) > (planned.details?.screens_here ?? 0)
                    text: `${planned.details?.widgets ?? 0} widgets replace yours, on ${planned.details?.screens ?? 0} screens` + ((planned.details?.screens ?? 0) > (planned.details?.screens_here ?? 0) ? `; ${planned.details?.screens_here ?? 0} are connected, so some stay hidden` : "")
                }
                Fact {
                    visible: planned.details?.wallpaper === true
                    icon: "wallpaper"
                    text: "Sets its wallpaper, with awww or swww"
                }
                Fact {
                    visible: planned.details?.installed === true
                    icon: "update"
                    text: "It's installed already: this updates it"
                }
                Fact {
                    visible: (planned.details?.problem ?? null) !== null
                    icon: "error"
                    warn: true
                    text: planned.details?.problem ?? ""
                }
                Fact {
                    icon: "undo"
                    text: "Removing it on the Installed page puts back the settings and widgets it replaces"
                }

                Repeater {
                    model: planned.details?.plugins ?? []

                    Column {
                        required property var modelData

                        width: root.width
                        topPadding: Theme.spaceSmall
                        spacing: Theme.spaceTiny

                        Fact {
                            icon: "extension"
                            warn: modelData.problem !== undefined
                            text: modelData.problem !== undefined ? `Needs the plugin ${modelData.id}: ${modelData.problem}` : `Needs the plugin ${modelData.id}, from ${modelData.source}`
                        }

                        PluginFacts {
                            visible: modelData.plan !== undefined
                            x: Theme.spaceLarge
                            width: root.width - Theme.spaceLarge
                            facts: modelData.plan ?? {}
                        }
                    }
                }
            }

            Row {
                x: Theme.spaceMedium
                topPadding: Theme.spaceSmall
                spacing: Theme.spaceSmall

                Button {
                    text: "Install"
                    icon: "download"
                    tone: "accent"
                    enabled: !root.busy && !planned.blocked
                    onClicked: Daemon.command("settings", "bento-add", [planned.plan.source, planned.plan.at ?? ""].filter(word => word !== ""))
                }

                Button {
                    visible: planned.plan.kind !== "plugin"
                    text: "Try"
                    icon: "visibility"
                    enabled: !root.busy
                    onClicked: Daemon.command("settings", "bento-try", [planned.plan.source])
                }

                Button {
                    text: "Cancel"
                    tone: "ghost"
                    onClicked: Daemon.command("settings", "bento-forget", [])
                }
            }
        }
    }

    // Discover: a package's page.
    Column {
        visible: root.page === "bento" && !root.planning && root.chosen !== null
        width: root.width
        spacing: Theme.spaceSmall

        Button {
            text: "All packages"
            icon: "arrow_back"
            tone: "ghost"
            onClicked: root.selected = ""
        }

        ListView {
            visible: (root.chosen?.screenshots ?? []).length > 0
            width: root.width
            height: 220
            orientation: ListView.Horizontal
            spacing: Theme.spaceSmall
            clip: true
            model: root.chosen?.screenshots ?? []

            delegate: Image {
                required property string modelData

                height: 220
                width: implicitHeight > 0 ? implicitWidth * height / implicitHeight : 360
                source: modelData
                fillMode: Image.PreserveAspectFit
                asynchronous: true
            }
        }

        Text {
            x: Theme.spaceMedium
            text: `${root.chosen?.name ?? ""} ${root.chosen?.version ?? ""}`
            color: Theme.foreground
            font.pixelSize: Theme.textHeadline
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Note {
            color: Theme.foreground
            text: root.chosen?.description ?? ""
        }

        Fact {
            icon: root.kindIcon(root.chosen?.kind ?? "")
            text: `${root.kindName(root.chosen?.kind ?? "")}, ${root.chosen?.license ?? ""}, looked after by ${(root.chosen?.maintainers ?? []).join(", ")}`
        }
        Fact {
            icon: "code"
            text: root.chosen?.repository ?? ""
        }
        Fact {
            visible: (root.chosen?.tags ?? []).length > 0
            icon: "sell"
            text: (root.chosen?.tags ?? []).join(", ")
        }
        Fact {
            visible: (root.chosen?.problem ?? null) !== null
            icon: "error"
            warn: true
            text: root.chosen?.problem ?? ""
        }

        Row {
            x: Theme.spaceMedium
            topPadding: Theme.spaceSmall
            spacing: Theme.spaceSmall

            Button {
                text: root.chosen?.installed ? "Install again" : "Install"
                icon: "download"
                tone: "accent"
                enabled: (root.chosen?.problem ?? null) === null
                onClicked: Daemon.command("settings", "bento-plan", [root.chosen.id])
            }

            Button {
                visible: root.chosen?.kind !== "plugin"
                text: "Try"
                icon: "visibility"
                enabled: !root.busy && (root.chosen?.problem ?? null) === null
                onClicked: Daemon.command("settings", "bento-try", [root.chosen.id])
            }
        }
    }

    // Discover: the registry.
    Column {
        visible: root.page === "bento" && !root.planning && root.chosen === null
        width: root.width
        spacing: Theme.spaceMedium

        Row {
            width: root.width
            spacing: Theme.spaceSmall

            Field {
                width: parent.width - kinds.width - reload.width - Theme.spaceSmall * 2
                hint: "Search the registry"
                onTextChanged: root.query = text
            }

            Segmented {
                id: kinds

                width: 360
                height: Theme.controlHeight
                current: root.kind
                options: [
                    {
                        "value": "all",
                        "label": "All"
                    },
                    {
                        "value": "plugin",
                        "label": "Plugins"
                    },
                    {
                        "value": "widget",
                        "label": "Widgets"
                    },
                    {
                        "value": "theme",
                        "label": "Themes"
                    },
                    {
                        "value": "bento",
                        "label": "Bentos"
                    }
                ]
                onPicked: value => root.kind = value
            }

            IconButton {
                id: reload

                anchors.verticalCenter: parent.verticalCenter
                icon: "refresh"
                enabled: root.bento.loading !== true
                onClicked: root.load(true)
            }
        }

        Note {
            visible: root.bento.loading === true && root.packages.length === 0
            text: "Reading the registry…"
        }

        Note {
            visible: (root.catalog.error ?? null) !== null
            color: Theme.danger
            text: `The registry can't be read: ${root.catalog.error ?? ""}`
        }

        Note {
            visible: root.bento.loading !== true && root.catalog.packages !== undefined && root.packages.filter(root.shown).length === 0 && (root.catalog.error ?? null) === null
            text: root.packages.length === 0 ? "The registry lists nothing yet." : "Nothing in the registry matches."
        }

        Grid {
            id: grid

            columns: 2
            spacing: Theme.spaceSmall
            width: root.width

            Repeater {
                model: root.packages.filter(root.shown)

                Rectangle {
                    id: card

                    required property var modelData
                    readonly property string shot: (modelData.screenshots ?? [])[0] ?? ""

                    width: (grid.width - grid.spacing) / 2
                    height: 196
                    radius: Theme.radiusField
                    color: area.containsMouse ? Theme.raised : Theme.surface
                    clip: true

                    Rectangle {
                        id: picture

                        width: parent.width
                        height: 112
                        color: Theme.raised

                        Image {
                            anchors.fill: parent
                            visible: card.shot !== ""
                            source: card.shot
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                        }

                        Symbol {
                            anchors.centerIn: parent
                            visible: card.shot === ""
                            name: root.kindIcon(card.modelData.kind)
                            size: 40
                            color: Theme.muted
                        }
                    }

                    Column {
                        anchors.top: picture.bottom
                        anchors.topMargin: Theme.spaceSmall
                        x: Theme.spaceMedium
                        width: parent.width - Theme.spaceMedium * 2
                        spacing: 2

                        Row {
                            width: parent.width
                            spacing: Theme.spaceSmall

                            Text {
                                width: parent.width - kindText.width - Theme.spaceSmall
                                text: `${card.modelData.name} ${card.modelData.version ?? ""}`
                                elide: Text.ElideRight
                                color: Theme.foreground
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                                font.weight: Theme.weightTitle
                            }

                            Text {
                                id: kindText

                                text: card.modelData.installed ? "Installed" : root.kindName(card.modelData.kind)
                                color: card.modelData.installed ? Theme.accent : Theme.muted
                                font.pixelSize: Theme.textCaption
                                font.family: Theme.fontFamily
                            }
                        }

                        Text {
                            width: parent.width
                            text: card.modelData.description || card.modelData.id
                            wrapMode: Text.Wrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                            color: Theme.muted
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.selected = card.modelData.id
                    }
                }
            }
        }

        // Anything else: a directory, a repository, a gist.
        Column {
            width: root.width
            spacing: Theme.spaceSmall

            Note {
                text: "Or install from a directory, a git repository or a gist's link:"
            }

            Row {
                width: root.width
                spacing: Theme.spaceSmall

                Field {
                    id: link

                    width: parent.width - look.width - Theme.spaceSmall
                    hint: "github.com/someone/cozy"
                    onAccepted: look.clicked()
                }

                Button {
                    id: look

                    anchors.verticalCenter: parent.verticalCenter
                    text: "Look"
                    enabled: link.text.trim() !== ""
                    onClicked: Daemon.command("settings", "bento-plan", [link.text.trim()])
                }
            }
        }
    }

    // Installed.
    Column {
        visible: root.page === "bento-installed"
        width: root.width
        spacing: 2

        Row {
            x: Theme.spaceMedium
            bottomPadding: Theme.spaceSmall
            spacing: Theme.spaceSmall
            visible: root.installed.some(entry => entry.update)

            Button {
                text: "Update all"
                icon: "update"
                tone: "accent"
                enabled: !root.busy
                onClicked: Daemon.command("settings", "bento-update", [])
            }
        }

        Note {
            visible: root.installed.length === 0 && (root.catalog.error ?? null) !== null && root.bento.loading !== true
            color: Theme.danger
            text: root.catalog.error ?? ""
        }

        Note {
            visible: root.installed.length === 0 && ((root.catalog.error ?? null) === null || root.bento.loading === true)
            text: root.bento.loading === true ? "Reading what Bento installed…" : "Bento installed nothing yet. Discover has what the registry lists."
        }

        Repeater {
            model: root.installed

            ListRow {
                id: row

                required property var modelData
                property bool confirming: false

                width: root.width
                height: Theme.rowHeight + (modelData.withdrawn ? 16 : 0)
                flat: true
                leadingSize: 26
                icon: root.kindIcon(modelData.kind)
                title: `${modelData.name} ${modelData.version ?? ""}`
                subtitle: modelData.withdrawn ? `<font color="${Theme.danger}">${modelData.withdrawn}</font><br>${modelData.source}` : modelData.by ? `${modelData.source}, with ${modelData.by}` : modelData.source
                subtitleFormat: Text.StyledText
                onClicked: confirming = false

                trailing: Row {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.spaceSmall

                    Button {
                        visible: (row.modelData.update ?? null) !== null
                        text: `Update to ${row.modelData.update}`
                        enabled: !root.busy
                        onClicked: Daemon.command("settings", "bento-update", [row.modelData.id])
                    }

                    Button {
                        visible: !row.modelData.by
                        text: row.confirming ? "Remove it?" : "Remove"
                        tone: row.confirming ? "danger" : "ghost"
                        enabled: !root.busy
                        onClicked: {
                            if (row.confirming)
                                Daemon.command("settings", "bento-remove", [row.modelData.id]);
                            row.confirming = !row.confirming;
                        }
                    }
                }
            }
        }
    }

    // Share.
    Column {
        visible: root.page === "bento-share"
        width: root.width
        spacing: Theme.spaceSmall

        Note {
            text: "A bento is a directory: push it to a git repository, or paste its mochi-bento.toml into a gist, and anyone installs it with `mochi bento add`. Device names, where you are, secrets and paths in your home are left out, and listed."
        }

        Field {
            id: folder

            width: root.width
            hint: "~/my-bento, the directory to write; its name is the bento's id"
            text: "~/my-bento"
        }

        Field {
            id: title

            width: root.width
            hint: "What it's called, like Cozy desk"
        }

        SwitchRow {
            id: wallpaper

            width: root.width
            icon: "wallpaper"
            title: "Bring the wallpaper"
            subtitle: "The image awww, swww or hyprpaper shows"
            onToggled: on => checked = on
        }

        Button {
            x: Theme.spaceMedium
            text: root.bento.sharing === true ? "Writing…" : "Make the bento"
            icon: "ios_share"
            tone: "accent"
            enabled: root.bento.sharing !== true && folder.text.trim() !== ""
            onClicked: Daemon.command("settings", "bento-share", [folder.text.trim(), wallpaper.checked ? "true" : "false", title.text.trim()].filter(word => word !== ""))
        }

        Note {
            visible: (root.bento.shared?.error ?? null) !== null
            color: Theme.danger
            text: root.bento.shared?.error ?? ""
        }

        Column {
            visible: (root.bento.shared?.dir ?? null) !== null
            width: root.width
            spacing: Theme.spaceTiny

            Fact {
                icon: "check_circle"
                text: `Wrote ${root.bento.shared?.dir ?? ""}`
            }

            Repeater {
                model: root.bento.shared?.summary ?? []

                Fact {
                    required property string modelData

                    icon: "inventory_2"
                    text: modelData
                }
            }

            Note {
                visible: (root.bento.shared?.left ?? []).length > 0
                topPadding: Theme.spaceSmall
                text: "Left out:"
            }

            Repeater {
                model: root.bento.shared?.left ?? []

                Fact {
                    required property string modelData

                    icon: "remove_circle"
                    text: modelData
                }
            }

            Note {
                topPadding: Theme.spaceSmall
                text: `Add a description and screenshots in ${root.bento.shared?.manifest ?? "mochi-bento.toml"}, then push the directory to a git repository.`
            }
        }
    }
}

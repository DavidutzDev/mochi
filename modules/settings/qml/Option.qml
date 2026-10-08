import QtQuick
import Quickshell
import qs.island

// One option: its title and description, and the control its kind calls
// for. An option changed here has a dot and, on hover, a button back to
// what the files say. A theme option goes out as it moves, at most every 50 ms; any
// other waits until it stops changing for 400 ms, since it restarts its
// module. `popup` asks the panel for a menu or the color picker, which it
// draws over everything.
//
// A text or a list whose values come from somewhere, its `source`, picks
// them from a menu, with a search and room to type another: apps, audio
// devices, tray apps, players, hub cards, modules. A command, like the idle
// clock's click, picks a module, then one of its actions, then its
// arguments. A command line with ready-made ones, like the lock, offers
// those that are installed.
Item {
    id: root

    required property var field
    // Shown before the title while searching, like "CPU".
    property string prefix: ""
    property string error: ""
    property bool highlighted: false
    signal popup(string kind, Item anchor)
    signal editToml

    readonly property bool group: field.kind === "group"
    readonly property bool theme: field.path.startsWith("theme.")
    // Changed in the panel: the reset button takes it back to the files.
    readonly property bool modified: !group && field.changed === true
    // The value the control shows: what's on the way, or the daemon's.
    property var pending: undefined
    readonly property var value: pending !== undefined ? pending : field.value
    readonly property bool ranged: field.min !== undefined && field.max !== undefined
    readonly property string source: field.source ?? ""
    // Picked from a menu rather than typed.
    readonly property bool picks: source !== "" || (field.kind === "list" && (field.choices ?? []).length > 0)

    // Which menu the panel shows for it: its own values, or for a command
    // the module or the action, or the ready-made commands.
    property string menu: "value"
    readonly property bool multiple: menu === "value" && field.kind === "list"
    readonly property bool searchable: menu === "value" && (source !== "" || choices().length > 8)
    // Something not in the menu may be typed too.
    readonly property bool custom: menu === "value" && ["app", "audio-device", "tray-app", "player"].includes(source)

    function openMenu(kind: string, anchor: Item): void {
        menu = kind;
        popup("choice", anchor);
    }

    // A command's module and action, and the arguments after them.
    readonly property string commandModule: source === "command" ? ((value ?? [])[0] ?? "") : ""
    readonly property string commandAction: source === "command" ? ((value ?? [])[1] ?? "") : ""
    readonly property var commandSpec: (Daemon.state("settings")?.actions?.[commandModule] ?? []).find(action => action.name === commandAction) ?? null

    // The daemon's answer replaces what was on the way.
    onFieldChanged: {
        if (!sending.running && !settle.running)
            pending = undefined;
    }

    function send(value: var): void {
        sending.stop();
        settle.stop();
        Daemon.command("settings", "set", [field.path, JSON.stringify(value)]);
    }

    // For values that move, like a slider being dragged.
    function sendSoon(value: var): void {
        pending = value;
        if (theme) {
            if (!sending.running)
                sending.start();
        } else {
            settle.restart();
        }
    }

    function sendNow(value: var): void {
        pending = value;
        send(value);
    }

    Timer {
        id: sending

        interval: 50
        onTriggered: root.send(root.pending)
    }

    Timer {
        id: settle

        interval: 400
        onTriggered: root.send(root.pending)
    }

    implicitHeight: group ? heading.implicitHeight + Theme.spaceMedium : Math.max(texts.implicitHeight, control.height) + Theme.spaceMedium * 2

    Rectangle {
        anchors.fill: parent
        visible: !root.group
        radius: Theme.radiusField
        color: root.highlighted ? Theme.raised : hover.hovered ? Theme.surface : "transparent"

        Behavior on color {
            ColorAnimation {
                duration: Theme.fast
            }
        }
    }

    HoverHandler {
        id: hover
    }

    // A table's heading, like "CPU" over its levels.
    Column {
        id: heading

        visible: root.group
        y: Theme.spaceMedium
        x: Theme.spaceMedium
        width: parent.width - Theme.spaceMedium * 2
        spacing: Theme.spaceTiny

        Text {
            text: root.field.title
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Text {
            visible: text !== ""
            width: parent.width
            text: root.field.description
            wrapMode: Text.Wrap
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }

    Column {
        id: texts

        visible: !root.group
        x: Theme.spaceMedium
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - x - Theme.spaceMedium * 2 - controls.width
        spacing: 2

        Row {
            spacing: Theme.spaceSmall

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.modified
                width: 6
                height: 6
                radius: width / 2
                color: Theme.accent
            }

            Text {
                text: root.prefix !== "" ? `${root.prefix} › ${root.field.title}` : root.field.title
                color: Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }
        }

        Text {
            visible: text !== ""
            width: parent.width
            text: root.field.description
            wrapMode: Text.Wrap
            maximumLineCount: 4
            elide: Text.ElideRight
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Text {
            visible: root.error !== ""
            width: parent.width
            text: root.error
            wrapMode: Text.Wrap
            color: Theme.danger
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }

    Row {
        id: controls

        visible: !root.group
        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMedium
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceSmall

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.modified
            opacity: hover.hovered ? 1 : 0
            icon: "restart_alt"
            size: 16
            onClicked: Daemon.command("settings", "reset", [root.field.path])
        }

        Loader {
            id: control

            anchors.verticalCenter: parent.verticalCenter
            sourceComponent: {
                switch (root.field.kind) {
                case "bool":
                    return toggleControl;
                case "choice":
                    return root.field.choices.length <= 4 && root.field.choices.every(choice => choice.value.length <= 10) ? segmentsControl : dropdownControl;
                case "int":
                case "float":
                    return root.ranged ? sliderControl : numberControl;
                case "color":
                    return colorControl;
                case "font":
                    return fontControl;
                case "list":
                    return root.source === "command" ? commandControl : listControl;
                case "text":
                    return root.source !== "" ? pickControl : textControl;
                default:
                    return tableControl;
                }
            }
        }
    }

    // "focus" reads "Focus", "center-left" "Center left".
    function label(value: string): string {
        if (value === "")
            return "Auto";
        const words = value.replace(/[-_]/g, " ");
        return words.charAt(0).toUpperCase() + words.slice(1);
    }

    // The values a source offers now, as {value, label, detail, icon}.
    function sourceChoices(): var {
        switch (source) {
        case "module":
            return (Daemon.state("settings")?.modules ?? []).map(module => ({
                        "value": module.id,
                        "label": module.title,
                        "icon": module.icon
                    }));
        case "app":
            return DesktopEntries.applications.values.filter(entry => !entry.noDisplay).map(entry => ({
                        "value": entry.id.replace(/\.desktop$/, ""),
                        "label": entry.name,
                        "detail": entry.id.replace(/\.desktop$/, ""),
                        "icon": entry.icon
                    })).sort((a, b) => a.label.localeCompare(b.label));
        case "audio-device":
            {
                const audio = Daemon.state("audio");
                return [
                    {
                        "value": "default_output",
                        "label": "Default output",
                        "detail": "What the speakers play"
                    },
                    {
                        "value": "default_input",
                        "label": "Default input",
                        "detail": "The microphone in use"
                    }
                ].concat((audio?.outputs ?? []).map(output => ({
                            "value": `${output.name}.monitor`,
                            "label": output.description,
                            "detail": "What it plays"
                        }))).concat((audio?.inputs ?? []).map(input => ({
                            "value": input.name,
                            "label": input.description,
                            "detail": "Microphone"
                        })));
            }
        case "tray-app":
            return (Daemon.state("tray")?.apps ?? []).map(app => ({
                        "value": app.id,
                        "label": app.title,
                        "detail": app.id,
                        "icon": app.icon
                    }));
        case "player":
            return (Daemon.state("media")?.players ?? []).map(player => ({
                        "value": player.name,
                        "label": player.name
                    }));
        case "hub-card":
            return Daemon.offered("hub", "card").map(card => ({
                        "value": `${card.module}/${card.id}`,
                        "label": card.title,
                        "detail": card.module,
                        "icon": card.icon
                    }));
        }
        return [];
    }

    // What a value reads as: its label in the menu, or itself.
    function labelFor(value: var): string {
        const text = String(value);
        const found = (source !== "" ? sourceChoices() : choices()).find(choice => choice.value === text);
        return found?.label ?? text;
    }

    function choices(): var {
        switch (menu) {
        case "module":
            return Object.keys(Daemon.state("settings")?.actions ?? {}).filter(module => (Daemon.state("settings").actions[module] ?? []).length > 0).sort().map(module => ({
                        "value": module,
                        "label": label(module)
                    }));
        case "action":
            return (Daemon.state("settings")?.actions?.[commandModule] ?? []).map(action => ({
                        "value": action.name,
                        "label": action.name,
                        "detail": action.help ?? action.description ?? ""
                    }));
        case "suggest":
            return (field.suggestions ?? []).map(command => ({
                        "value": JSON.stringify(command),
                        "label": command.join(" ")
                    }));
        }
        if (source !== "")
            return sourceChoices();
        const options = (root.field.choices ?? []).map(choice => ({
                    "value": choice.value,
                    "label": label(choice.value),
                    "colors": choice.colors ?? []
                }));
        return root.field.optional ? [
            {
                "value": "",
                "label": "Auto"
            }
        ].concat(options) : options;
    }

    function isPicked(choice: string): bool {
        switch (menu) {
        case "module":
            return commandModule === choice;
        case "action":
            return commandAction === choice;
        case "suggest":
            return JSON.stringify(value ?? []) === choice;
        }
        if (multiple)
            return (value ?? []).map(String).includes(choice);
        return (value ?? "") === choice;
    }

    // A menu's choice: a list gains or loses it and the menu stays; the
    // rest set it.
    function pick(choice: string): void {
        switch (menu) {
        case "module":
            sendNow(choice === commandModule ? value : [choice]);
            return;
        case "action":
            sendNow([commandModule, choice]);
            return;
        case "suggest":
            sendNow(JSON.parse(choice));
            return;
        }
        if (multiple) {
            const items = (value ?? []).slice();
            const at = items.map(String).indexOf(choice);
            if (at >= 0)
                items.splice(at, 1);
            else
                items.push(field.items === "int" || field.items === "float" ? Number(choice) : choice);
            sendNow(items);
            return;
        }
        sendNow(choice === "" ? null : choice);
    }

    Component {
        id: toggleControl

        Switch {
            checked: root.value === true
            onToggled: checked => root.sendNow(checked)
        }
    }

    Component {
        id: segmentsControl

        Segmented {
            width: root.choices().length * 76
            height: Theme.controlHeight
            options: root.choices()
            current: root.value ?? ""
            onPicked: value => root.sendNow(value === "" ? null : value)
        }
    }

    Component {
        id: dropdownControl

        Rectangle {
            id: button

            width: 180
            height: Theme.controlHeight
            radius: height / 2
            color: pick.containsMouse ? Theme.highlight : Theme.raised

            Text {
                x: Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                text: root.label(root.value ?? "")
                color: Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }

            Symbol {
                anchors.right: parent.right
                anchors.rightMargin: Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                name: "chevron"
                rotation: 90
                size: 12
                color: Theme.muted
            }

            MouseArea {
                id: pick

                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.openMenu("value", button)
            }
        }
    }

    // The font's name, in that font.
    Component {
        id: fontControl

        Rectangle {
            id: button

            readonly property string family: root.value ?? ""

            width: 240
            height: Theme.controlHeight
            radius: height / 2
            color: fontArea.containsMouse ? Theme.highlight : Theme.raised

            Text {
                x: Theme.spaceMedium
                width: parent.width - x - Theme.spaceHuge
                anchors.verticalCenter: parent.verticalCenter
                elide: Text.ElideRight
                text: button.family !== "" ? button.family : "Default"
                color: button.family !== "" && !Fonts.has(button.family) ? Theme.danger : Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: button.family !== "" ? button.family : Theme.fontFamily
            }

            Symbol {
                anchors.right: parent.right
                anchors.rightMargin: Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                name: "chevron"
                rotation: 90
                size: 12
                color: Theme.muted
            }

            MouseArea {
                id: fontArea

                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.popup("font", button)
            }
        }
    }

    Component {
        id: sliderControl

        Row {
            spacing: Theme.spaceSmall

            readonly property real span: root.field.max - root.field.min

            Slider {
                id: bar

                anchors.verticalCenter: parent.verticalCenter
                width: 160
                thickness: 4
                fill: Theme.accent
                value: ((root.value ?? root.field.min) - root.field.min) / parent.span
                // A double click goes back to what the files say.
                reset: ((root.field.saved ?? root.field["default"] ?? root.field.min) - root.field.min) / parent.span
                onMoved: value => root.sendSoon(parent.snap(value))
                onReleased: value => root.sendNow(parent.snap(value))
            }

            function snap(fraction: real): var {
                const value = root.field.min + fraction * span;
                return root.field.kind === "int" ? Math.round(value) : Math.round(value * 100) / 100;
            }

            Entry {
                anchors.verticalCenter: parent.verticalCenter
                width: 64
                text: String(root.value ?? "")
                horizontalAlignment: TextInput.AlignRight
                onAccepted: text => root.typed(text)
            }
        }
    }

    // A number typed in, kept if it is one.
    function typed(text: string): void {
        const number = Number(text.trim());
        if (text.trim() === "" && root.field.optional)
            sendNow(null);
        else if (!Number.isNaN(number))
            sendNow(root.field.kind === "int" ? Math.round(number) : number);
    }

    // Steps of about a tenth of the default: 100 for 1500 ms, 1 for 50.
    readonly property real step: {
        const base = Math.abs(field["default"] ?? 0);
        if (field.kind === "float")
            return base >= 10 ? 1 : 0.1;
        return base >= 100 ? Math.pow(10, Math.floor(Math.log10(base)) - 1) : 1;
    }

    Component {
        id: numberControl

        Row {
            spacing: Theme.spaceTiny

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                icon: "remove"
                size: 14
                onClicked: {
                    const next = (root.value ?? 0) - root.step;
                    root.sendSoon(Math.max(root.field.min ?? -Infinity, Math.round(next * 100) / 100));
                }
            }

            Entry {
                anchors.verticalCenter: parent.verticalCenter
                width: 84
                text: root.value === null || root.value === undefined ? "" : String(root.value)
                placeholder: root.field.optional ? "Auto" : ""
                horizontalAlignment: TextInput.AlignHCenter
                onAccepted: text => root.typed(text)
            }

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                icon: "add"
                size: 14
                onClicked: {
                    const next = (root.value ?? 0) + root.step;
                    root.sendSoon(Math.min(root.field.max ?? Infinity, Math.round(next * 100) / 100));
                }
            }
        }
    }

    Component {
        id: textControl

        Entry {
            width: 240
            text: root.value ?? ""
            placeholder: root.field["default"] === "" ? "Default" : ""
            onAccepted: text => root.sendNow(text)
        }
    }

    Component {
        id: colorControl

        Row {
            spacing: Theme.spaceSmall

            Rectangle {
                id: swatch

                anchors.verticalCenter: parent.verticalCenter
                width: Theme.controlHeight
                height: Theme.controlHeight
                radius: Theme.radiusControl
                color: root.value ?? "transparent"
                border.width: 1
                border.color: Theme.highlight

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.popup("color", swatch)
                }
            }

            Entry {
                anchors.verticalCenter: parent.verticalCenter
                width: 110
                mono: true
                text: root.value ?? ""
                onAccepted: text => root.sendNow(text.trim())
            }
        }
    }

    Component {
        id: listControl

        Column {
            width: 280
            spacing: Theme.spaceTiny

            Flow {
                width: parent.width
                spacing: Theme.spaceTiny

                Repeater {
                    model: root.value ?? []

                    Rectangle {
                        id: chip

                        required property var modelData
                        required property int index

                        width: chipText.implicitWidth + remove.width + Theme.spaceSmall * 2
                        height: 26
                        radius: height / 2
                        color: Theme.raised

                        Text {
                            id: chipText

                            x: Theme.spaceSmall
                            anchors.verticalCenter: parent.verticalCenter
                            text: root.labelFor(chip.modelData)
                            color: Theme.foreground
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }

                        IconButton {
                            id: remove

                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            icon: "close"
                            size: 12
                            onClicked: {
                                const items = (root.value ?? []).slice();
                                items.splice(chip.index, 1);
                                root.sendNow(items);
                            }
                        }
                    }
                }
            }

            // Values from the menu, several at a time.
            DropButton {
                id: menuButton1

                visible: root.picks
                width: parent.width
                text: "Add…"
                onClicked: root.openMenu("value", menuButton1)
            }

            // Ready-made commands, the installed ones.
            DropButton {
                id: menuButton2

                visible: (root.field.suggestions ?? []).length > 0
                width: parent.width
                text: "Pick a ready-made one…"
                onClicked: root.openMenu("suggest", menuButton2)
            }

            Entry {
                visible: !root.picks
                width: parent.width
                placeholder: (root.field.suggestions ?? []).length > 0 ? "Or type a word…" : "Add…"
                onAccepted: added => {
                    const item = added.trim();
                    clear();
                    if (item === "")
                        return;
                    const number = Number(item);
                    const numeric = root.field.items === "int" || root.field.items === "float";
                    if (numeric && Number.isNaN(number))
                        return;
                    root.sendNow((root.value ?? []).concat([numeric ? number : item]));
                }
            }
        }
    }

    // A button that opens a menu: its text, and a chevron.
    component DropButton: Rectangle {
        id: drop

        property string text: ""
        property bool muted: false
        signal clicked

        width: 180
        height: Theme.controlHeight
        radius: height / 2
        color: dropArea.containsMouse ? Theme.highlight : Theme.raised
        opacity: enabled ? 1 : 0.4

        Text {
            x: Theme.spaceMedium
            width: parent.width - x - Theme.spaceHuge
            anchors.verticalCenter: parent.verticalCenter
            text: drop.text
            elide: Text.ElideRight
            color: drop.muted ? Theme.muted : Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Symbol {
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            name: "chevron"
            rotation: 90
            size: 12
            color: Theme.muted
        }

        MouseArea {
            id: dropArea

            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: drop.clicked()
        }
    }

    // One value from a source, or typed in the menu's search.
    Component {
        id: pickControl

        DropButton {
            id: menuButton3

            width: 240
            text: root.value ? root.labelFor(root.value) : root.field.optional || root.field["default"] === "" ? "Default" : "Choose…"
            muted: !root.value
            onClicked: root.openMenu("value", menuButton3)
        }
    }

    // A command: a module, one of its actions, then the action's arguments.
    Component {
        id: commandControl

        Column {
            width: 280
            spacing: Theme.spaceTiny

            Row {
                spacing: Theme.spaceTiny

                DropButton {
                    id: menuButton4

                    width: 120
                    text: root.commandModule !== "" ? root.label(root.commandModule) : "Nothing"
                    muted: root.commandModule === ""
                    onClicked: root.openMenu("module", menuButton4)
                }

                DropButton {
                    id: menuButton5

                    width: 128
                    enabled: root.commandModule !== ""
                    text: root.commandAction !== "" ? root.commandAction : "Action…"
                    muted: root.commandAction === ""
                    onClicked: root.openMenu("action", menuButton5)
                }

                IconButton {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: (root.value ?? []).length > 0
                    icon: "close"
                    size: 14
                    onClicked: root.sendNow([])
                }
            }

            // What the action takes, named in the field.
            Entry {
                visible: (root.commandSpec?.args ?? []).length > 0
                width: parent.width
                text: (root.value ?? []).slice(2).join(" ")
                placeholder: (root.commandSpec?.args ?? []).map(arg => arg.optional ? `[${arg.name}]` : arg.name).join(" ")
                onAccepted: text => {
                    const args = text.trim() === "" ? [] : text.trim().split(/\s+/);
                    root.sendNow([root.commandModule, root.commandAction].concat(args));
                }
            }
        }
    }

    Component {
        id: tableControl

        Button {
            text: "Edit as TOML"
            icon: "code"
            onClicked: root.editToml()
        }
    }
}

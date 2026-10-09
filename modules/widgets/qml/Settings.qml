import QtQuick
import qs.island
import "Place.js" as Place

// The selected widget's settings, as a form made from what it declares:
// its looks, when it has more than one, then a switch for an on-off
// setting, segments for a few choices and chips for more, a field
// otherwise, leaving out those its look doesn't use. Each change goes to the module at once. With more than
// one monitor, a choice of monitor sends the widget to another one.
Rectangle {
    id: root

    required property Item desktop
    // The selected widget's frame, or null.
    required property Item frame

    readonly property var widget: frame?.widget ?? null
    readonly property var spec: widget ? desktop.catalog.find(entry => entry.module === widget.module && entry.widget === widget.widget) : null
    readonly property var variants: spec?.variants ?? []
    readonly property var variant: variants.find(variant => variant.id === widget?.variant) ?? null
    readonly property var fields: (spec?.settings ?? []).filter(field => !variant?.settings || variant.settings.includes(field.name))
    // The monitors as they're laid out, left to right.
    readonly property var screens: Daemon.screens.slice().sort((a, b) => a.x - b.x || a.y - b.y)

    // Sends the widget to another monitor with its anchor and offsets,
    // moved in as far as it takes to be all on that screen, and arranging
    // follows it there with its settings open.
    function send(output: string): void {
        const screen = screens.find(screen => screen.name === output);
        if (!screen || !widget || output === widget.output)
            return;
        const spot = Place.onScreen(widget, desktop.cell, screen.width, screen.height);
        Daemon.command("widgets", "move", [widget.id, output, spot.anchor, `${spot.x}`, `${spot.y}`]);
        Daemon.command("widgets", "edit", ["on", output, widget.id]);
    }

    visible: frame !== null && desktop.editing
    width: 300
    height: column.implicitHeight + Theme.padding * 2
    radius: Theme.radiusSurface
    color: Theme.background
    border.width: 1
    border.color: Theme.border
    // Beside the widget, on whichever side has room, and on screen.
    x: {
        if (!frame)
            return 0;
        const right = frame.x + frame.width + 12;
        const left = frame.x - width - 12;
        return right + width <= desktop.width ? right : Math.max(0, left);
    }
    y: frame ? Math.max(12, Math.min(desktop.height - height - 12, frame.y)) : 0

    // Clicks here stay here.
    MouseArea {
        anchors.fill: parent
    }

    EdgeLight {
        radius: root.radius
    }

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: parent.width - Theme.padding * 2
        spacing: Theme.spaceMedium

        PanelHeader {
            width: parent.width
            title: root.spec?.title ?? ""
        }

        // Its looks: a new one comes at its own size.
        Column {
            visible: root.variants.length > 1
            width: parent.width
            spacing: Theme.spaceSmall

            Column {
                width: parent.width
                spacing: 2

                Text {
                    text: "look"
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.variant?.description ?? ""
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }
            }

            Flow {
                width: parent.width
                spacing: Theme.spaceSmall

                Repeater {
                    model: root.variants

                    Rectangle {
                        id: look

                        required property var modelData
                        readonly property bool chosen: root.variant?.id === modelData.id

                        function pick(): void {
                            if (!chosen)
                                Daemon.command("widgets", "variant", [root.widget.id, modelData.id]);
                        }

                        width: lookLabel.implicitWidth + Theme.spaceLarge + Theme.spaceTiny
                        height: Theme.controlHeight - Theme.spaceTiny
                        radius: height / 2
                        color: chosen ? Theme.foreground : lookArea.containsMouse ? Theme.highlight : Theme.raised
                        activeFocusOnTab: true
                        Keys.onReturnPressed: pick()
                        Keys.onEnterPressed: pick()
                        Keys.onSpacePressed: pick()

                        Text {
                            id: lookLabel

                            anchors.centerIn: parent
                            text: look.modelData.title
                            color: look.chosen ? Theme.background : Theme.foreground
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                        }

                        MouseArea {
                            id: lookArea

                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: look.pick()
                        }

                        Rectangle {
                            visible: look.activeFocus
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

        Text {
            visible: root.fields.length === 0
            width: parent.width
            wrapMode: Text.Wrap
            text: "It has no settings."
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Repeater {
            model: root.fields

            Column {
                id: field

                required property var modelData
                readonly property var value: root.widget?.settings?.[modelData.name] ?? modelData.default
                readonly property var choices: modelData.choices ?? []

                width: column.width
                spacing: Theme.spaceSmall

                function set(value: string): void {
                    Daemon.command("widgets", "set", [root.widget.id, modelData.name, value]);
                }

                Item {
                    width: parent.width
                    height: Math.max(label.implicitHeight, toggle.visible ? toggle.height : 0)

                    Column {
                        id: label

                        width: parent.width - (toggle.visible ? toggle.width + 12 : 0)
                        spacing: 2

                        Text {
                            text: field.modelData.name
                            color: Theme.foreground
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                        }

                        Text {
                            visible: text !== ""
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: field.modelData.description ?? ""
                            color: Theme.muted
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }
                    }

                    Switch {
                        id: toggle

                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        visible: field.modelData.kind === "bool"
                        checked: field.value === true
                        onToggled: checked => field.set(checked ? "true" : "false")
                    }
                }

                Segmented {
                    visible: field.modelData.kind === "choice" && field.choices.length <= 4
                    width: parent.width
                    height: 34
                    color: Theme.raised
                    options: (field.modelData.choices ?? []).map(choice => ({
                                "value": choice,
                                "label": choice
                            }))
                    current: `${field.value}`
                    keyboard: true
                    onPicked: value => field.set(value)
                }

                // More than a row of segments holds: chips that wrap, the
                // current one filled, like the looks.
                Flow {
                    visible: field.modelData.kind === "choice" && field.choices.length > 4
                    width: parent.width
                    spacing: Theme.spaceTiny

                    Repeater {
                        model: field.choices

                        Rectangle {
                            id: chip

                            required property string modelData
                            readonly property bool chosen: `${field.value}` === modelData

                            function pick(): void {
                                if (!chosen)
                                    field.set(modelData);
                            }

                            width: chipLabel.implicitWidth + Theme.spaceLarge
                            height: Theme.controlHeight - Theme.spaceSmall
                            radius: height / 2
                            color: chosen ? Theme.foreground : chipArea.containsMouse ? Theme.highlight : Theme.raised
                            activeFocusOnTab: true
                            Keys.onReturnPressed: pick()
                            Keys.onEnterPressed: pick()
                            Keys.onSpacePressed: pick()

                            Text {
                                id: chipLabel

                                anchors.centerIn: parent
                                text: chip.modelData
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
                                onClicked: chip.pick()
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

                Rectangle {
                    visible: ["string", "int", "float"].includes(field.modelData.kind)
                    width: parent.width
                    height: 34
                    radius: Theme.radiusControl
                    color: Theme.raised
                    border.width: input.activeFocus ? 1 : 0
                    border.color: Theme.accent

                    TextInput {
                        id: input

                        anchors.fill: parent
                        anchors.leftMargin: Theme.spaceSmall
                        anchors.rightMargin: Theme.spaceSmall
                        verticalAlignment: TextInput.AlignVCenter
                        text: `${field.value ?? ""}`
                        color: Theme.foreground
                        selectionColor: Theme.accent
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                        clip: true
                        // Saved on Enter or when leaving the field.
                        onEditingFinished: {
                            if (text !== `${field.value ?? ""}`)
                                field.set(text);
                        }
                    }
                }
            }
        }

        Column {
            visible: root.screens.length > 1
            width: parent.width
            spacing: Theme.spaceSmall

            Column {
                width: parent.width
                spacing: 2

                Text {
                    text: "monitor"
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: "The screen it's on; pick another to send it there"
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }
            }

            Segmented {
                width: parent.width
                height: 34
                color: Theme.raised
                options: root.screens.map(screen => ({
                            "value": screen.name,
                            "label": screen.name
                        }))
                current: root.widget?.output ?? ""
                onPicked: value => root.send(value)
            }
        }

        Button {
            text: "Remove"
            icon: "trash"
            tone: "danger"
            onClicked: {
                Daemon.command("widgets", "remove", [root.widget.id]);
                root.desktop.selected = "";
            }
        }
    }
}

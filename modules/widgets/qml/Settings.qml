import QtQuick
import qs.island

// The selected widget's settings, as a form made from what it declares:
// a switch for an on-off setting, a choice for a few, a field otherwise.
// Each change goes to the module at once.
Rectangle {
    id: root

    required property Item desktop
    // The selected widget's frame, or null.
    required property Item frame

    readonly property var widget: frame?.widget ?? null
    readonly property var spec: widget ? desktop.catalog.find(entry => entry.module === widget.module && entry.widget === widget.widget) : null
    readonly property var fields: spec?.settings ?? []

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
                    visible: field.modelData.kind === "choice"
                    width: parent.width
                    height: 34
                    color: Theme.raised
                    options: (field.modelData.choices ?? []).map(choice => ({
                                "value": choice,
                                "label": choice
                            }))
                    current: `${field.value}`
                    onPicked: value => field.set(value)
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

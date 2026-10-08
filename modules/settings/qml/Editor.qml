import QtQuick
import qs.island

// A section as TOML, every option at its value. Save checks it first and
// shows what's wrong instead of changing anything.
Item {
    id: root

    // The section's path, like config.module.osd.
    required property string path
    // What the module sent for it: {path, text}, or null until it comes.
    property var sent: null
    property string error: ""
    signal closed

    property bool dirty: false

    onSentChanged: {
        if (sent && sent.path === path && !dirty)
            edit.text = sent.text;
    }
    Component.onCompleted: {
        if (sent && sent.path === path)
            edit.text = sent.text;
    }

    // `[module.osd]` for config.module.osd, `[colors]` for theme.colors.
    readonly property string table: path.split(".").slice(1).join(".")
    readonly property string file: path.startsWith("theme.") ? "theme.toml" : "config.toml"

    Column {
        anchors.fill: parent
        spacing: Theme.spaceSmall

        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: `The [${root.table}] table of ${root.file}, every option at its value. Options you remove go back to what your files say.`
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Rectangle {
            width: parent.width
            height: parent.height - y - buttons.height - (problem.visible ? problem.height + Theme.spaceSmall : 0) - Theme.spaceSmall
            radius: Theme.radiusField
            color: Theme.surface
            border.width: edit.activeFocus ? 1 : 0
            border.color: Theme.accent

            Flickable {
                id: view

                anchors.fill: parent
                anchors.margins: Theme.spaceMedium
                contentWidth: edit.contentWidth
                contentHeight: edit.contentHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                function follow(cursor: rect): void {
                    if (cursor.y < contentY)
                        contentY = cursor.y;
                    else if (cursor.y + cursor.height > contentY + height)
                        contentY = cursor.y + cursor.height - height;
                }

                TextEdit {
                    id: edit

                    width: Math.max(implicitWidth, view.width)
                    height: Math.max(implicitHeight, view.height)
                    color: Theme.foreground
                    selectionColor: Theme.accent
                    selectedTextColor: Theme.onAccent
                    selectByMouse: true
                    font.family: "monospace"
                    font.pixelSize: Theme.textBody
                    tabStopDistance: font.pixelSize * 2
                    onTextChanged: {
                        if (activeFocus)
                            root.dirty = true;
                    }
                    onCursorRectangleChanged: view.follow(cursorRectangle)
                    Component.onCompleted: Qt.callLater(() => forceActiveFocus())
                    Keys.onEscapePressed: event => {
                        root.closed();
                        event.accepted = true;
                    }
                    // Ctrl+S saves, as anywhere else.
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_S && event.modifiers & Qt.ControlModifier) {
                            root.save();
                            event.accepted = true;
                        }
                    }
                }
            }
        }

        Text {
            id: problem

            visible: root.error !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: root.error
            color: Theme.danger
            font.pixelSize: Theme.textCaption
            // TOML's errors point at a column with a caret.
            font.family: "monospace"
        }

        Row {
            id: buttons

            anchors.right: parent.right
            spacing: Theme.spaceSmall

            Button {
                text: "Cancel"
                tone: "ghost"
                onClicked: root.closed()
            }

            Button {
                text: "Save"
                icon: "check"
                tone: "accent"
                onClicked: root.save()
            }
        }
    }

    // What was typed stays until the panel closes the editor, also when
    // the daemon refuses it.
    function save(): void {
        Daemon.command("settings", "edit", [path, edit.text]);
    }
}

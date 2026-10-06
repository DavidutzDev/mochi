import QtQuick
import qs.island

// A to-do list widget: its title and how many are done, the items, a click
// marks one done, and a field at the bottom adds one. Each placed widget
// has its own list, kept by the notes module under its instance id.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    // It takes text, so the desktop gives it the keyboard on a click.
    readonly property bool typing: true

    readonly property var items: payload?.pages?.[instance]?.items ?? []
    // With their place in the whole list, which the actions take.
    readonly property var shown: items.map((item, index) => Object.assign({ index: index }, item)).filter(item => settings.done !== "hide" || !item.done)
    readonly property int done: items.filter(item => item.done).length

    Row {
        id: header

        width: parent.width
        spacing: 8

        Text {
            anchors.baseline: count.baseline
            text: (root.settings.title ?? "To-do").toUpperCase()
            color: Theme.accent
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
            font.letterSpacing: 1
        }

        Text {
            id: count

            visible: root.items.length > 0
            text: `${root.done} of ${root.items.length} done`
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }

    Text {
        anchors.right: parent.right
        anchors.verticalCenter: header.verticalCenter
        visible: root.done > 0
        text: "Clear done"
        color: clear.containsMouse ? Theme.foreground : Theme.muted
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily

        MouseArea {
            id: clear

            anchors.fill: parent
            anchors.margins: -4
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: Daemon.command("notes", "clear", [root.instance])
        }
    }

    ListView {
        id: list

        anchors.top: header.bottom
        anchors.topMargin: 10
        anchors.bottom: add.top
        anchors.bottomMargin: 8
        width: parent.width
        clip: true
        spacing: 2
        model: root.shown
        boundsBehavior: Flickable.StopAtBounds

        delegate: Item {
            id: row

            required property var modelData

            width: list.width
            height: Math.max(26, label.implicitHeight + 8)

            MouseArea {
                id: hover

                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: Daemon.command("notes", "toggle", [root.instance, `${row.modelData.index}`])
            }

            Rectangle {
                id: box

                anchors.verticalCenter: parent.verticalCenter
                width: 16
                height: 16
                radius: 8
                color: row.modelData.done ? Theme.accent : "transparent"
                border.width: row.modelData.done ? 0 : 1.5
                border.color: Theme.muted

                Symbol {
                    anchors.centerIn: parent
                    visible: row.modelData.done
                    name: "check"
                    size: 11
                    color: Theme.onAccent
                }
            }

            Text {
                id: label

                anchors.left: box.right
                anchors.leftMargin: 10
                anchors.right: remove.left
                anchors.rightMargin: 6
                anchors.verticalCenter: parent.verticalCenter
                text: row.modelData.text
                wrapMode: Text.Wrap
                color: row.modelData.done ? Theme.muted : Theme.foreground
                font.strikeout: row.modelData.done
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }

            Symbol {
                id: remove

                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                opacity: hover.containsMouse || removing.containsMouse ? 1 : 0
                name: "close"
                size: 12
                color: removing.containsMouse ? Theme.danger : Theme.muted

                MouseArea {
                    id: removing

                    anchors.fill: parent
                    anchors.margins: -6
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Daemon.command("notes", "delete", [root.instance, `${row.modelData.index}`])
                }
            }
        }
    }

    // Adding: Enter adds what's typed, Escape lets go of the keyboard.
    Item {
        id: add

        anchors.bottom: parent.bottom
        width: parent.width
        height: 28

        Symbol {
            id: plus

            anchors.verticalCenter: parent.verticalCenter
            name: "plus"
            size: 14
            color: Theme.muted
        }

        TextInput {
            id: input

            anchors.left: plus.right
            anchors.leftMargin: 10
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.foreground
            selectionColor: Theme.accent
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            clip: true
            onAccepted: {
                if (text.trim() !== "")
                    Daemon.command("notes", "add", [root.instance, text]);
                text = "";
            }
            Keys.onEscapePressed: {
                text = "";
                focus = false;
            }

            Text {
                visible: input.text === "" && !input.activeFocus
                text: "Add an item"
                color: Theme.muted
                font: input.font
            }
        }
    }
}

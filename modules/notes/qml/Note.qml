import QtQuick
import qs.island

// A note widget: free text, typed straight into it on the desktop. It's
// saved a moment after typing stops, and when the note loses the keyboard.
// Each placed note has its own text, kept by the notes module.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    // It takes text, so the desktop gives it the keyboard on a click.
    readonly property bool typing: true

    readonly property string saved: payload?.pages?.[instance]?.text ?? ""
    readonly property string title: settings.title ?? "Note"

    // The saved text shows unless it's being edited, so a save coming back
    // never moves the cursor.
    onSavedChanged: {
        if (!edit.activeFocus && edit.text !== saved)
            edit.text = saved;
    }
    Component.onCompleted: edit.text = saved

    function save(): void {
        pause.stop();
        if (edit.text !== saved)
            Daemon.command("notes", "write", edit.text === "" ? [instance] : [instance, edit.text]);
    }

    Timer {
        id: pause

        interval: 800
        onTriggered: root.save()
    }

    Text {
        id: heading

        visible: root.title !== ""
        text: root.title.toUpperCase()
        color: Theme.accent
        font.pixelSize: Theme.textLabel
        font.family: Theme.fontFamily
        font.weight: Font.DemiBold
        font.letterSpacing: 1
    }

    Flickable {
        id: scroll

        anchors.top: heading.visible ? heading.bottom : parent.top
        anchors.topMargin: heading.visible ? 10 : 0
        anchors.bottom: parent.bottom
        width: parent.width
        clip: true
        contentHeight: edit.implicitHeight
        boundsBehavior: Flickable.StopAtBounds

        TextEdit {
            id: edit

            width: scroll.width
            // The whole note takes clicks, not only the lines written.
            height: Math.max(implicitHeight, scroll.height)
            wrapMode: TextEdit.Wrap
            color: Theme.foreground
            selectionColor: Theme.accent
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            onTextChanged: {
                if (activeFocus)
                    pause.restart();
            }
            onActiveFocusChanged: {
                if (!activeFocus)
                    root.save();
            }
            onCursorRectangleChanged: {
                // Keeps the cursor in view while typing past the bottom.
                if (cursorRectangle.y + cursorRectangle.height > scroll.contentY + scroll.height)
                    scroll.contentY = cursorRectangle.y + cursorRectangle.height - scroll.height;
                else if (cursorRectangle.y < scroll.contentY)
                    scroll.contentY = cursorRectangle.y;
            }
            Keys.onEscapePressed: focus = false

            Text {
                visible: edit.text === "" && !edit.activeFocus
                text: "Click to write"
                color: Theme.muted
                font: edit.font
            }
        }
    }
}

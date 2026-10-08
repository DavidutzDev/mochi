import QtQuick
import qs.island

// A one-line text field: shows `text`, and says `accepted` with what was
// typed on Enter or when it loses focus. Escape puts `text` back.
Rectangle {
    id: root

    property string text: ""
    property string placeholder: ""
    property bool mono: false
    property alias horizontalAlignment: input.horizontalAlignment
    property alias validator: input.validator
    readonly property alias editing: input.activeFocus
    // Says `edited` on every keystroke too, like a search box.
    property bool live: false
    signal accepted(string text)
    signal edited(string text)

    implicitWidth: 180
    implicitHeight: Theme.controlHeight
    radius: Theme.radiusControl
    color: Theme.raised
    border.width: input.activeFocus ? 1 : 0
    border.color: Theme.accent

    onTextChanged: {
        if (!input.activeFocus)
            input.text = text;
    }

    function clear(): void {
        input.text = "";
    }

    function commit(): void {
        if (input.text !== root.text)
            root.accepted(input.text);
    }

    // Focusing the field focuses its text.
    onActiveFocusChanged: {
        if (activeFocus)
            input.forceActiveFocus();
    }

    TextInput {
        id: input

        anchors.fill: parent
        leftPadding: Theme.spaceSmall
        rightPadding: Theme.spaceSmall
        verticalAlignment: TextInput.AlignVCenter
        clip: true
        selectByMouse: true
        color: Theme.foreground
        selectionColor: Theme.accent
        selectedTextColor: Theme.onAccent
        font.pixelSize: Theme.textBody
        font.family: root.mono ? "monospace" : Theme.fontFamily
        Component.onCompleted: text = root.text

        onTextChanged: {
            if (root.live)
                root.edited(text);
        }
        // A live field only takes Enter: leaving it isn't a choice.
        onAccepted: root.live ? root.accepted(text) : root.commit()
        onActiveFocusChanged: {
            if (!activeFocus && !root.live)
                root.commit();
        }
        Keys.onEscapePressed: event => {
            text = root.text;
            focus = false;
            event.accepted = true;
        }

        Text {
            anchors.fill: parent
            leftPadding: Theme.spaceSmall
            verticalAlignment: Text.AlignVCenter
            visible: input.text === "" && !input.activeFocus
            text: root.placeholder
            color: Theme.muted
            font: input.font
        }
    }
}

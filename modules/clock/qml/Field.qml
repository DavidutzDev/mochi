import QtQuick
import qs.island

// A one-line text field on a raised fill, with a ring in the accent color
// while it has the keyboard. Tab reaches it, Enter sends `accepted`, and
// Escape goes on to the panel, which closes.
Rectangle {
    id: root

    property alias text: input.text
    property string placeholder: ""
    property alias maximumLength: input.maximumLength
    signal accepted

    function focusInput(): void {
        input.forceActiveFocus();
        input.selectAll();
    }

    implicitWidth: 160
    implicitHeight: Theme.controlHeight
    radius: Theme.radiusField
    color: Theme.raised
    border.width: input.activeFocus ? 2 : 0
    border.color: Theme.accent

    TextInput {
        id: input

        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMedium
        anchors.verticalCenter: parent.verticalCenter
        activeFocusOnTab: true
        clip: true
        color: Theme.foreground
        selectionColor: Theme.accent
        selectedTextColor: Theme.onAccent
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
        Keys.onReturnPressed: root.accepted()
        Keys.onEnterPressed: root.accepted()

        Text {
            visible: input.text === ""
            text: root.placeholder
            color: Theme.muted
            font: input.font
        }
    }

    // A click anywhere on the fill puts the cursor in.
    MouseArea {
        anchors.fill: parent
        z: -1
        cursorShape: Qt.IBeamCursor
        onClicked: input.forceActiveFocus()
    }
}

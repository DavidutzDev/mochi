import QtQuick

// A Button the keyboard reaches with Tab and presses with Enter or Space,
// with a ring around it while it has focus.
Item {
    id: root

    property alias text: button.text
    property alias icon: button.icon
    property alias tone: button.tone
    signal clicked

    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight
    activeFocusOnTab: enabled && visible
    Keys.onReturnPressed: root.clicked()
    Keys.onEnterPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()

    Button {
        id: button

        anchors.fill: parent
        onClicked: root.clicked()
    }

    Rectangle {
        visible: root.activeFocus
        anchors.fill: parent
        anchors.margins: -3
        radius: height / 2
        color: "transparent"
        border.width: 2
        border.color: Theme.accent
    }
}

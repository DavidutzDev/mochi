import QtQuick

// A credit at the bottom of a module's settings page: whose work inspired
// the module, and "See repo", which opens their repository through the
// settings module's `open-link` action. `link` names one of the pages that
// action knows, like "mochi-clock".
Column {
    id: root

    property var payload: ({})
    property string author: ""
    property string link: ""

    spacing: 2

    Text {
        width: root.width
        wrapMode: Text.Wrap
        text: `This module is inspired by the work of ${root.author}.`
        color: Theme.muted
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
    }

    Text {
        id: repo

        text: "See repo"
        color: area.containsMouse || repo.activeFocus ? Theme.foreground : Theme.muted
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
        font.underline: true
        activeFocusOnTab: true
        Keys.onReturnPressed: root.open()
        Keys.onEnterPressed: root.open()
        Keys.onSpacePressed: root.open()

        MouseArea {
            id: area

            anchors.fill: parent
            anchors.margins: -Theme.spaceTiny
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.open()
        }
    }

    function open(): void {
        Daemon.command("settings", "open-link", [root.link]);
    }
}

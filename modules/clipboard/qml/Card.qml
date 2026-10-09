import QtQuick
import qs.island

// The control center card: how much the history holds and where, with buttons
// to pause it and to clear it.
Item {
    id: root

    property var payload: null
    readonly property int count: payload?.count ?? 0
    readonly property bool paused: payload?.paused ?? false

    implicitHeight: Theme.rowHeight

    Column {
        anchors.left: parent.left
        anchors.right: buttons.left
        anchors.rightMargin: Theme.spaceSmall
        anchors.verticalCenter: parent.verticalCenter

        RollingText {
            text: {
                const pins = root.payload?.pins ?? 0;
                const count = root.paused ? "Paused" : root.count === 1 ? "1 entry" : `${root.count} entries`;
                return pins > 0 ? `${count} · ${pins} pinned` : count;
            }
            pixelSize: Theme.textBody
            weight: Theme.weightTitle
        }

        Text {
            width: parent.width
            text: root.payload?.warning ?? (root.payload?.storage === "disk" ? "Encrypted on disk" : "In memory until you log out")
            elide: Text.ElideRight
            color: root.payload?.warning ? Theme.danger : Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }

    Row {
        id: buttons

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        IconButton {
            icon: root.paused ? "play" : "pause"
            size: 16
            onClicked: Daemon.command("clipboard", "pause", ["toggle"])
        }

        IconButton {
            icon: "trash"
            size: 16
            enabled: root.count > 0
            opacity: enabled ? 1 : 0.4
            onClicked: Daemon.command("clipboard", "clear", [])
        }
    }
}

import QtQuick
import qs.island

// The player's name, between arrows that switch to the other players when
// several have a track. The one picked stays shown until it stops.
Item {
    id: root

    property var payload: ({})
    property int fontSize: Theme.textCaption
    property color color: Theme.muted
    readonly property bool several: (payload?.players?.length ?? 0) > 1

    implicitWidth: (several ? previous.width + next.width + 4 : 0) + name.implicitWidth
    implicitHeight: several ? previous.height : name.implicitHeight

    IconButton {
        id: previous

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        visible: root.several
        icon: "chevron"
        rotation: 180
        size: 12
        tone: "ghost"
        onClicked: Daemon.command("media", "previous-player", [])
    }

    Text {
        id: name

        anchors.left: root.several ? previous.right : parent.left
        anchors.leftMargin: root.several ? 2 : 0
        anchors.right: root.several ? next.left : parent.right
        anchors.rightMargin: root.several ? 2 : 0
        anchors.verticalCenter: parent.verticalCenter
        text: root.payload?.player ?? ""
        elide: Text.ElideRight
        color: root.color
        font.pixelSize: root.fontSize
        font.family: Theme.fontFamily
    }

    IconButton {
        id: next

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        visible: root.several
        icon: "chevron"
        size: 12
        tone: "ghost"
        onClicked: Daemon.command("media", "next-player", [])
    }
}

import QtQuick
import qs.island

// A round control that runs a media action.
Item {
    id: root

    required property string icon
    required property string action
    property real size: 20
    property bool enabled: true

    implicitWidth: size + 16
    implicitHeight: size + 16
    opacity: enabled ? 1 : 0.35

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: Theme.surface
        opacity: area.containsMouse && root.enabled ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: 120
            }
        }
    }

    Glyph {
        anchors.centerIn: parent
        name: root.icon
        size: root.size
        scale: area.pressed && root.enabled ? 0.88 : 1

        Behavior on scale {
            NumberAnimation {
                duration: 100
            }
        }
    }

    MouseArea {
        id: area

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: {
            if (root.enabled)
                Daemon.command("media", root.action, []);
        }
    }
}

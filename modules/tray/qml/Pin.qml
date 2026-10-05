import QtQuick
import qs.island

// A pinned app's own bubble: its icon. A click activates the app, a right
// click opens its menu, a middle click does its second action, and the
// wheel scrolls on it. It breathes while the app asks for attention.
Item {
    id: root

    property var payload: ({})
    readonly property string key: payload.key ?? ""

    implicitWidth: 26
    implicitHeight: 26

    AppIcon {
        id: icon

        anchors.centerIn: parent
        icon: root.payload.icon ?? ""
        size: 18

        SequentialAnimation on opacity {
            running: root.payload.attention ?? false
            loops: Animation.Infinite
            onRunningChanged: {
                if (!running)
                    icon.opacity = 1;
            }

            NumberAnimation {
                to: 0.35
                duration: 900
                easing.type: Easing.InOutSine
            }

            NumberAnimation {
                to: 1
                duration: 900
                easing.type: Easing.InOutSine
            }
        }
    }

    // Over the pill's own area, so every button lands here.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        cursorShape: Qt.PointingHandCursor
        onClicked: mouse => {
            if (mouse.button === Qt.RightButton)
                Daemon.command("tray", "menu", [root.key]);
            else if (mouse.button === Qt.MiddleButton)
                Daemon.command("tray", "secondary", [root.key]);
            else
                Daemon.command("tray", "activate", [root.key]);
        }
        onWheel: wheel => {
            const vertical = Math.abs(wheel.angleDelta.y) >= Math.abs(wheel.angleDelta.x);
            const delta = vertical ? wheel.angleDelta.y : wheel.angleDelta.x;
            if (delta !== 0)
                Daemon.command("tray", "scroll", [root.key, `${delta}`, vertical ? "vertical" : "horizontal"]);
        }
    }
}

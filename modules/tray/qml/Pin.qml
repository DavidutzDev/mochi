import QtQuick
import Quickshell
import qs.island

// A pinned app's own bubble: its icon. A click activates the app, a right
// click opens its menu, a middle click does its second action, and the
// wheel scrolls on it. It breathes while the app asks for attention.
// Resting the pointer on it shows the app's tooltip beside it, away from
// the screen's edge.
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
                duration: Theme.duration(900)
                easing.type: Easing.InOutSine
            }

            NumberAnimation {
                to: 1
                duration: Theme.duration(900)
                easing.type: Easing.InOutSine
            }
        }
    }

    // Over the pill's own area, so every button lands here.
    MouseArea {
        id: area

        anchors.fill: parent
        hoverEnabled: true
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

    // The pointer rested a moment, so passing over doesn't flash it.
    Timer {
        id: rest

        interval: 500
        running: area.containsMouse && (root.payload.tooltip ?? "") !== ""
    }

    PopupWindow {
        readonly property bool below: Theme.anchor !== "bottom"

        anchor.item: root
        anchor.rect.y: below ? root.height + Theme.spaceSmall : -Theme.spaceSmall
        anchor.rect.x: root.width / 2
        anchor.edges: below ? Edges.Bottom : Edges.Top
        anchor.gravity: below ? Edges.Bottom : Edges.Top
        visible: area.containsMouse && !rest.running && (root.payload.tooltip ?? "") !== ""
        implicitWidth: Math.min(tip.implicitWidth, 280) + Theme.padding * 2
        implicitHeight: tip.implicitHeight + Theme.spaceSmall * 2
        color: "transparent"

        Rectangle {
            anchors.fill: parent
            radius: Theme.radiusField
            color: Theme.background
            border.color: Theme.border
            border.width: 1

            Text {
                id: tip

                x: Theme.padding
                y: Theme.spaceSmall
                width: Math.min(implicitWidth, 280)
                text: root.payload.tooltip ?? ""
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
                color: Theme.foreground
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }
}

import QtQuick
import qs.island

// The tray's bubble, which opens the drawer. It breathes in the accent color
// while an app asks for attention.
Item {
    id: root

    property var payload: ({})
    readonly property bool attention: payload.attention ?? false

    implicitWidth: 26
    implicitHeight: 26

    Symbol {
        id: symbol

        anchors.centerIn: parent
        name: "tray"
        size: 16
        color: root.attention ? Theme.accent : Theme.foreground

        SequentialAnimation on opacity {
            running: root.attention
            loops: Animation.Infinite
            onRunningChanged: {
                if (!running)
                    symbol.opacity = 1;
            }

            NumberAnimation {
                to: 0.4
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
}

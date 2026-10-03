import QtQuick

// How far along something is, from 0 to 1. Not interactive; Slider is.
Rectangle {
    id: root

    property real value: 0
    property color fill: Theme.foreground

    implicitWidth: 160
    implicitHeight: 4
    radius: height / 2
    color: Theme.raised

    Rectangle {
        width: root.width * Math.max(0, Math.min(root.value, 1))
        height: root.height
        radius: root.radius
        color: root.fill

        Behavior on width {
            NumberAnimation {
                duration: Theme.fast
                easing.type: Easing.OutCubic
            }
        }
    }
}

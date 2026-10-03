import QtQuick

// On or off. The owner flips `checked` when it hears `toggled`, so the
// switch always shows the real state.
Rectangle {
    id: root

    property bool checked: false
    signal toggled(bool checked)

    implicitWidth: 44
    implicitHeight: 26
    radius: height / 2
    color: checked ? Theme.accent : Theme.highlight
    opacity: enabled ? 1 : 0.4

    Behavior on color {
        ColorAnimation {
            duration: Theme.fast
        }
    }

    Rectangle {
        x: root.checked ? root.width - width - 3 : 3
        anchors.verticalCenter: parent.verticalCenter
        width: root.height - 6
        height: width
        radius: width / 2
        color: Theme.foreground

        Behavior on x {
            NumberAnimation {
                duration: Theme.move
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Theme.overshoot
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.enabled
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled(!root.checked)
    }
}

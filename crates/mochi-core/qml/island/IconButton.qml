import QtQuick

// A round icon button, sized from its icon, with no fill until hovered by
// default: media controls, close buttons. `tone` works as in Button.
Button {
    property real size: 18

    iconSize: size
    implicitHeight: size + 14
    tone: "ghost"
}

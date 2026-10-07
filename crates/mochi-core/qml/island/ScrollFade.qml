import QtQuick

// Soft edges for a list that scrolls: an edge with more content past it
// fades into `color`, and stays sharp at the start or the end. Put it
// inside the Flickable or ListView it belongs to, with `view` set to it.
Item {
    id: root

    required property Flickable view
    property bool horizontal: false
    property color color: Theme.background
    property real size: Theme.spaceLarge

    parent: view
    anchors.fill: parent
    z: 2
    enabled: false

    component Edge: Rectangle {
        property bool start
        property bool shown

        x: root.horizontal && !start ? root.width - root.size : 0
        y: !root.horizontal && !start ? root.height - root.size : 0
        width: root.horizontal ? root.size : root.width
        height: root.horizontal ? root.height : root.size
        opacity: shown ? 1 : 0
        gradient: Gradient {
            orientation: root.horizontal ? Gradient.Horizontal : Gradient.Vertical

            GradientStop {
                position: 0
                color: Qt.alpha(root.color, start ? 1 : 0)
            }

            GradientStop {
                position: 1
                color: Qt.alpha(root.color, start ? 0 : 1)
            }
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.fast
            }
        }
    }

    Edge {
        start: true
        shown: root.horizontal ? !root.view.atXBeginning : !root.view.atYBeginning
    }

    Edge {
        start: false
        shown: root.horizontal ? !root.view.atXEnd : !root.view.atYEnd
    }
}

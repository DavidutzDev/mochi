import QtQuick
import QtQuick.Shapes
import qs.island

// One session's mark: the agent with a short arc turning round it while it
// works, a raised hand on the accent while it waits for you, and a check
// once it's done.
Item {
    id: root

    // "working", "waiting" or "done".
    property string status: "working"
    property real size: 20

    implicitWidth: size
    implicitHeight: size

    Rectangle {
        anchors.fill: parent
        visible: root.status === "waiting"
        radius: width / 2
        color: Theme.accent
    }

    Symbol {
        anchors.centerIn: parent
        name: root.status === "waiting" ? "front_hand" : root.status === "done" ? "check" : "smart_toy"
        size: root.status === "done" ? root.size * 0.8 : root.size * 0.6
        color: root.status === "waiting" ? Theme.onAccent : root.status === "done" ? Theme.success : Theme.foreground
    }

    Shape {
        anchors.fill: parent
        visible: root.status === "working"
        preferredRendererType: Shape.CurveRenderer

        RotationAnimation on rotation {
            running: root.status === "working" && !Theme.reducedMotion
            from: 0
            to: 360
            duration: Theme.duration(1400)
            loops: Animation.Infinite
        }

        ShapePath {
            fillColor: "transparent"
            strokeColor: Theme.muted
            strokeWidth: 1.5
            capStyle: ShapePath.RoundCap

            PathAngleArc {
                centerX: root.size / 2
                centerY: root.size / 2
                radiusX: root.size / 2 - 1
                radiusY: root.size / 2 - 1
                startAngle: -90
                sweepAngle: 100
            }
        }
    }
}

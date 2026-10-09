import QtQuick
import QtQuick.Shapes
import qs.island

// How much of the phase is left, as a ring that empties clockwise from the
// top, over a faint track.
Shape {
    id: root

    property real progress: 1
    property color color: Theme.accent
    property real line: 2
    readonly property real radius: Math.min(width, height) / 2 - line / 2

    preferredRendererType: Shape.CurveRenderer

    ShapePath {
        fillColor: "transparent"
        strokeColor: Theme.raised
        strokeWidth: root.line

        PathAngleArc {
            centerX: root.width / 2
            centerY: root.height / 2
            radiusX: root.radius
            radiusY: root.radius
            startAngle: 0
            sweepAngle: 360
        }
    }

    ShapePath {
        fillColor: "transparent"
        strokeColor: root.color
        strokeWidth: root.line
        capStyle: ShapePath.RoundCap

        PathAngleArc {
            centerX: root.width / 2
            centerY: root.height / 2
            radiusX: root.radius
            radiusY: root.radius
            startAngle: -90
            sweepAngle: 360 * Math.max(0, Math.min(1, root.progress))
        }
    }
}

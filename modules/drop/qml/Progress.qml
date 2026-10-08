import QtQuick
import QtQuick.Shapes
import qs.island

// The bubble while an action on dropped files runs, so the panel can
// close meanwhile: what the files are as an icon, and a ring that fills as
// it goes. Before it can tell how far, the ring turns. A click opens the
// panel again, where Stop ends it.
Item {
    id: root

    property var payload: ({})
    readonly property real progress: payload.progress ?? 0
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: {
        const parts = [`${payload.doing ?? "Working"}: ${payload.files ?? ""}`];
        if (progress > 0)
            parts.push(`${Math.round(progress * 100)}%`);
        return parts.join(" · ") + "\nClick to see it, or stop it";
    }

    implicitWidth: 26
    implicitHeight: 26

    Symbol {
        anchors.centerIn: parent
        name: root.payload.icon ?? "draft"
        size: 13
        color: Theme.foreground
    }

    Shape {
        id: ring

        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        // Not knowing how far yet: a short arc going round.
        rotation: 0

        RotationAnimation on rotation {
            running: root.progress <= 0
            from: 0
            to: 360
            duration: Theme.duration(1200)
            loops: Animation.Infinite
        }

        ShapePath {
            fillColor: "transparent"
            strokeColor: Theme.raised
            strokeWidth: 2

            PathAngleArc {
                centerX: 13
                centerY: 13
                radiusX: 12
                radiusY: 12
                startAngle: 0
                sweepAngle: 360
            }
        }

        ShapePath {
            fillColor: "transparent"
            strokeColor: Theme.accent
            strokeWidth: 2
            capStyle: ShapePath.RoundCap

            PathAngleArc {
                centerX: 13
                centerY: 13
                radiusX: 12
                radiusY: 12
                startAngle: -90
                sweepAngle: root.progress > 0 ? 360 * root.progress : 70

                Behavior on sweepAngle {
                    NumberAnimation {
                        duration: Theme.fast
                    }
                }
            }
        }
    }

    onProgressChanged: {
        if (progress > 0)
            ring.rotation = 0;
    }
}

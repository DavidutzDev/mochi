import QtQuick
import QtQuick.Shapes
import Quickshell.Widgets
import qs.island

// The music while the island shows something else: the cover in a circle,
// with the track's progress as a ring around it. Paused, the cover dims.
Item {
    id: root

    property var payload: ({})

    implicitWidth: 26
    implicitHeight: 26

    Position {
        id: clock

        payload: root.payload
    }

    ClippingRectangle {
        anchors.centerIn: parent
        width: 20
        height: 20
        radius: width / 2
        color: Theme.raised
        opacity: clock.playing ? 1 : 0.5

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.fast
            }
        }

        Symbol {
            anchors.centerIn: parent
            visible: cover.status !== Image.Ready
            name: "note"
            size: 12
            color: Theme.muted
        }

        Image {
            id: cover

            anchors.fill: parent
            source: root.payload.art ?? ""
            sourceSize.width: 40
            sourceSize.height: 40
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
        }
    }

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        // The track behind the progress.
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
            strokeColor: clock.playing ? Theme.accent : Theme.muted
            strokeWidth: 2
            capStyle: ShapePath.RoundCap

            PathAngleArc {
                centerX: 13
                centerY: 13
                radiusX: 12
                radiusY: 12
                startAngle: -90
                sweepAngle: 360 * clock.progress
            }
        }
    }
}

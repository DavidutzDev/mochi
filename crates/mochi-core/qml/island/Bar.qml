import QtQuick
import QtQuick.Effects
import QtQuick.Shapes

// Bar mode's strip along the whole edge, as tall as the idle island, with
// the bubbles on it and the island in it. The island and the pills draw no
// background of their own then: this draws one outline for the strip and
// whatever part of the island hangs past it, with ears where the island
// leaves the strip, so a panel grows out of the bar like a notch.
//
// The geometry is worked out with the bar at the top; `flipY` mirrors it to
// the bottom edge. It fills the window, in the window's coordinates.
Item {
    id: root

    required property Island island
    property bool flipY: false
    // The strip's thickness.
    property real thickness: Theme.idleHeight

    readonly property real w: width
    readonly property real h: thickness

    // The island's sides, where it ends away from the edge, unflipped, and
    // how far past the strip that is.
    readonly property real start: island.x
    readonly property real end: island.x + island.width
    readonly property real far: flipY ? height - island.y : island.y + island.height
    readonly property real hang: island.shown ? Math.max(0, far - h) : 0
    // The hanging part's corners, and its ears, grow with it from nothing,
    // as in IslandShape.
    readonly property real corner: Math.min(island.radius, hang)
    readonly property real ear: Math.min(Theme.earRadius, Math.max(0, hang - corner), Math.max(0, start), Math.max(0, w - end))
    readonly property bool hanging: hang > 0.5

    function point(x: real, y: real): string {
        return `${x} ${flipY ? height - y : y}`;
    }

    // Mirroring reverses the direction of every arc.
    function arc(r: real, convex: bool, x: real, y: real): string {
        const sweep = convex !== flipY ? 1 : 0;
        return ` A ${r} ${r} 0 0 ${sweep} ${point(x, y)}`;
    }

    // The side away from the edge, clockwise from the right end: the strip's
    // edge, and around the island where it hangs past it.
    readonly property string inner: {
        let path = `M ${point(w, h)}`;
        if (hanging) {
            path += ` L ${point(end + ear, h)}` + arc(ear, false, end, h + ear);
            path += ` L ${point(end, far - corner)}` + arc(corner, true, end - corner, far);
            path += ` L ${point(start + corner, far)}` + arc(corner, true, start, far - corner);
            path += ` L ${point(start, h + ear)}` + arc(ear, false, start - ear, h);
        }
        return path + ` L ${point(0, h)}`;
    }

    // A soft shadow under the strip and under the island where it hangs,
    // both covered by the shape where they overlap it. Theme.shadow sets
    // its strength, and transparent turns it off.
    RectangularShadow {
        visible: Theme.shadow.a > 0
        y: root.flipY ? root.height - root.h : 0
        width: root.w
        height: root.h
        blur: 16
        offset.y: root.flipY ? -2 : 2
        color: Theme.shadow
    }

    RectangularShadow {
        visible: Theme.shadow.a > 0 && root.hanging
        x: root.island.x
        y: root.island.y
        width: root.island.width
        height: root.island.height
        radius: root.corner
        blur: 16
        offset.y: root.flipY ? -2 : 2
        color: Theme.shadow
    }

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: Theme.background
            strokeColor: "transparent"

            PathSvg {
                path: `${root.inner} L ${root.point(0, 0)} L ${root.point(root.w, 0)} Z`
            }
        }

        // A hairline along the side away from the edge; one along the
        // screen's edge would only show as a stray line there.
        ShapePath {
            fillColor: "transparent"
            strokeColor: Theme.border
            strokeWidth: 1
            capStyle: ShapePath.FlatCap
            joinStyle: ShapePath.RoundJoin

            PathSvg {
                path: root.inner
            }
        }
    }
}

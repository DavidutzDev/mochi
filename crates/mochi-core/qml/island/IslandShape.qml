import QtQuick
import QtQuick.Shapes

// The island's outline, drawn as one path so it can morph between a floating
// pill and a notch attached to the screen edge.
//
// The geometry is worked out with the attached edge at the top and the
// attached side, if any, on the left; `flipX` and `flipY` mirror it to the
// real anchor. Ears are concave corners that flare into an edge. They sit
// outside the item's bounds.
Shape {
    id: root

    // Radius of the free corners.
    property real radius: 0
    // 0 floating, 1 touching the edge. Values in between morph.
    property real attached: 0
    // 0 or 1: also touching the side edge, in a corner.
    property real sideAttached: 0
    property real earRadius: 0
    property bool flipX: false
    property bool flipY: false
    property color color: "black"

    readonly property real w: width
    readonly property real h: height

    // Ears never go past the straight part of the side they curve into.
    readonly property real edgeEar: Math.min(earRadius, Math.max(0, h - radius)) * attached
    readonly property real sideEar: Math.min(earRadius, Math.max(0, w - radius)) * sideAttached

    // Unflipped corners and ears.
    readonly property real topLeft: radius * (1 - attached)
    readonly property real topRight: radius * (1 - attached)
    readonly property real bottomRight: radius
    readonly property real bottomLeft: radius * (1 - sideAttached)
    readonly property real leftEar: edgeEar * (1 - sideAttached)
    readonly property real rightEar: edgeEar
    readonly property real downEar: sideEar

    // Corner radii as drawn, for the input mask and the blur region.
    readonly property var corners: {
        const radii = {
            "top-left": topLeft,
            "top-right": topRight,
            "bottom-right": bottomRight,
            "bottom-left": bottomLeft
        };
        const at = (top, left) => radii[`${top ? "top" : "bottom"}-${left ? "left" : "right"}`];
        return {
            topLeft: at(!flipY, !flipX),
            topRight: at(!flipY, flipX),
            bottomRight: at(flipY, flipX),
            bottomLeft: at(flipY, !flipX)
        };
    }

    // Each ear as the square it fills, with the circle cut out of it, in item
    // coordinates.
    readonly property var ears: [
        ear(-leftEar, 0, leftEar, -leftEar, leftEar),
        ear(w, 0, rightEar, w + rightEar, rightEar),
        ear(0, h, downEar, downEar, h + downEar)
    ].filter(ear => ear.size > 0.5)

    function ear(x: real, y: real, size: real, cx: real, cy: real): var {
        return {
            x: flipX ? w - x - size : x,
            y: flipY ? h - y - size : y,
            size: size,
            cx: flipX ? w - cx : cx,
            cy: flipY ? h - cy : cy
        };
    }

    function point(x: real, y: real): string {
        return `${flipX ? w - x : x} ${flipY ? h - y : y}`;
    }

    // Mirroring one axis reverses the direction of every arc.
    function arc(r: real, convex: bool, x: real, y: real): string {
        const sweep = convex !== (flipX !== flipY) ? 1 : 0;
        return ` A ${r} ${r} 0 0 ${sweep} ${point(x, y)}`;
    }

    readonly property string path: {
        // Clockwise from the top-left corner.
        let path = leftEar > 0 ? `M ${point(-leftEar, 0)}` : `M ${point(topLeft, 0)}`;

        if (rightEar > 0)
            path += ` L ${point(w + rightEar, 0)}` + arc(rightEar, false, w, rightEar);
        else
            path += ` L ${point(w - topRight, 0)}` + arc(topRight, true, w, topRight);

        path += ` L ${point(w, h - bottomRight)}` + arc(bottomRight, true, w - bottomRight, h);

        if (downEar > 0)
            path += ` L ${point(downEar, h)}` + arc(downEar, false, 0, h + downEar);
        else
            path += ` L ${point(bottomLeft, h)}` + arc(bottomLeft, true, 0, h - bottomLeft);

        if (leftEar > 0)
            path += ` L ${point(0, leftEar)}` + arc(leftEar, false, -leftEar, 0);
        else
            path += ` L ${point(0, topLeft)}` + arc(topLeft, true, topLeft, 0);

        return path + " Z";
    }

    preferredRendererType: Shape.CurveRenderer

    ShapePath {
        fillColor: root.color
        strokeColor: "transparent"

        PathSvg {
            path: root.path
        }
    }
}

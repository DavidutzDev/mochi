import QtQuick
import QtQuick.Shapes

// A filled shape to set one thing apart, like today's date on a calendar
// or the icon of a weather widget: a circle, a pentagon with round corners,
// a cookie with soft lobes, a four-lobed clover, a burst with soft points,
// a hexagon or an octagon with round corners, a squircle (a square with
// soft sides), or a pill. It's an accent: one per widget, behind the thing
// that matters. What's put inside sits on top, centered if it anchors so,
// and fits in `roomWidth` by `roomHeight`. The shape fills the largest
// square that fits, with a lobe or a point at the top, or a flat side for
// the hexagon and the octagon; a pill takes the whole width, and up to
// 0.62 of it in height. `angle` turns the shape, and leaves what's inside
// upright.
Item {
    id: root

    // "circle", "pentagon", "cookie", "clover", "burst", "hexagon",
    // "octagon", "squircle" or "pill".
    property string shape: "cookie"
    property real size: 40
    property color color: Theme.accent
    // How many lobes a cookie has, or points a burst.
    property int lobes: shape === "burst" ? 12 : 9
    // Degrees the shape turns clockwise.
    property real angle: 0

    readonly property real side: Math.min(width, height)
    // A pill's height.
    readonly property real pillHeight: Math.min(height, width * 0.62)

    // The box in the middle where what's put inside fits, in pixels.
    readonly property real roomWidth: shape === "pill" ? width - pillHeight * 0.4 : side * (({
                "circle": 0.7,
                "pentagon": 0.6,
                "cookie": 0.66,
                "clover": 0.56,
                "burst": 0.6,
                "hexagon": 0.72,
                "octagon": 0.74,
                "squircle": 0.8
            })[shape] ?? 0.6)
    readonly property real roomHeight: shape === "pill" ? pillHeight * 0.72 : side * (({
                "circle": 0.7,
                "pentagon": 0.52,
                "cookie": 0.66,
                "clover": 0.56,
                "burst": 0.6,
                "hexagon": 0.6,
                "octagon": 0.74,
                "squircle": 0.8
            })[shape] ?? 0.6)

    implicitWidth: size
    implicitHeight: size

    // How far a lobe's edge dips between lobes, as a share of the radius.
    readonly property real depth: shape === "clover" ? 0.24 : shape === "burst" ? 0.14 : 0.08

    // `|value|` from 0 to 1 with its sharp bottom rounded off, so the dip
    // between two lobes is a soft notch instead of a point.
    function soft(value: real): real {
        const round = 0.03;
        const low = Math.sqrt(round);
        return (Math.sqrt(value * value + round) - low) / (Math.sqrt(1 + round) - low);
    }

    // The shape's radius at an angle measured from the top, for the shapes
    // drawn around a center, with 1 at the tips.
    function radius(angle: real): real {
        switch (shape) {
        case "clover":
            return 1 - depth * (1 - soft(Math.cos(angle * 2)));
        case "burst":
            return 1 - depth * soft(Math.sin(angle * lobes / 2));
        case "cookie":
            return 1 - depth * (1 - soft(Math.cos(angle * lobes / 2)));
        case "squircle":
            // |x|^4 + |y|^4 = 1, which reaches the square's sides.
            return Math.pow(Math.pow(Math.abs(Math.sin(angle)), 4) + Math.pow(Math.abs(Math.cos(angle)), 4), -0.25);
        default:
            return 1;
        }
    }

    function around(): string {
        const r = side / 2;
        const cx = width / 2;
        const cy = height / 2;
        // Points about every 3 pixels of the outline, and at least 8 a lobe.
        const steps = Math.min(360, Math.max(lobes * 8, Math.round(Math.PI * side / 3)));
        let path = "";
        for (let i = 0; i < steps; i++) {
            const angle = i / steps * 2 * Math.PI;
            const at = r * radius(angle);
            const x = cx + at * Math.sin(angle);
            const y = cy - at * Math.cos(angle);
            path += `${i === 0 ? "M" : " L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
        }
        return path + " Z";
    }

    // A regular polygon with round corners, pointing up, or with a flat
    // side up when `flat`, worked out at a radius of 1, then scaled and
    // moved so its outline fills the square.
    function polygon(sides: int, rounding: real, flat: bool): string {
        const corners = [];
        for (let i = 0; i < sides; i++) {
            const angle = -Math.PI / 2 + (flat ? Math.PI / sides : 0) + i * 2 * Math.PI / sides;
            corners.push([Math.cos(angle), Math.sin(angle)]);
        }
        const half = Math.PI / 2 - Math.PI / sides;
        // How far from a corner its curve starts, along each side.
        const along = rounding / Math.tan(half);
        // How far in from the corner the curve's center is.
        const inward = rounding / Math.sin(half);
        const toward = (from, to, by) => {
            const dx = to[0] - from[0];
            const dy = to[1] - from[1];
            const length = Math.hypot(dx, dy);
            return [from[0] + dx / length * by, from[1] + dy / length * by];
        };
        // The outline's bounds are the bounds of the corners' circles.
        let left = Infinity, right = -Infinity, top = Infinity, bottom = -Infinity;
        for (const corner of corners) {
            const center = toward(corner, [0, 0], inward);
            left = Math.min(left, center[0] - rounding);
            right = Math.max(right, center[0] + rounding);
            top = Math.min(top, center[1] - rounding);
            bottom = Math.max(bottom, center[1] + rounding);
        }
        const scale = side / Math.max(right - left, bottom - top);
        const ox = width / 2 - (left + right) / 2 * scale;
        const oy = height / 2 - (top + bottom) / 2 * scale;
        const point = p => `${(ox + p[0] * scale).toFixed(2)} ${(oy + p[1] * scale).toFixed(2)}`;
        const r = (rounding * scale).toFixed(2);
        let path = "";
        for (let i = 0; i <= sides; i++) {
            const corner = corners[i % sides];
            const before = corners[(i + sides - 1) % sides];
            const after = corners[(i + 1) % sides];
            if (i > 0)
                path += ` L ${point(toward(corner, before, along))} A ${r} ${r} 0 0 1 ${point(toward(corner, after, along))}`;
            else
                path += `M ${point(toward(corner, after, along))}`;
        }
        return path + " Z";
    }

    readonly property string outline: {
        if (side <= 0)
            return "";
        if (shape === "pentagon")
            return polygon(5, 0.3, false);
        if (shape === "hexagon")
            return polygon(6, 0.24, true);
        if (shape === "octagon")
            return polygon(8, 0.18, true);
        if (shape === "pill") {
            const r = pillHeight / 2;
            const top = height / 2 - r;
            const bottom = height / 2 + r;
            return `M ${r} ${top} L ${width - r} ${top} A ${r} ${r} 0 0 1 ${width - r} ${bottom} L ${r} ${bottom} A ${r} ${r} 0 0 1 ${r} ${top} Z`;
        }
        if (shape === "circle") {
            const r = side / 2;
            return `M ${width / 2 - r} ${height / 2} A ${r} ${r} 0 1 1 ${width / 2 + r} ${height / 2} A ${r} ${r} 0 1 1 ${width / 2 - r} ${height / 2} Z`;
        }
        return around();
    }

    // In an item of its own: a Shape sizes itself from its path, which
    // here follows the size.
    Shape {
        anchors.fill: parent
        rotation: root.angle
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: root.color
            strokeColor: "transparent"
            strokeWidth: 0

            PathSvg {
                path: root.outline
            }
        }
    }
}

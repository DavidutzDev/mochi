import QtQuick
import QtQuick.Shapes

// How full something is, from 0 to 1, as a ring that fills clockwise from
// the top over a flat track, like a battery's charge or a disk's use. The
// filled part is a gentle wave, or a plain arc with `wavy` off or the
// theme's waves off; a gap keeps it apart from the track. What's put inside
// sits in the ring's middle, like a percentage. Changes of `value` glide.
Item {
    id: root

    property real value: 0
    property real size: 48
    property real thickness: 4
    property color color: Theme.accent
    property color trackColor: Theme.raised
    property bool wavy: true
    // Waves around the whole ring; by default one about every 5.5 times
    // the line's width.
    property int waves: Math.max(5, Math.round(2 * Math.PI * radius / (thickness * 5.5)))
    default property alias content: middle.data

    readonly property real amplitude: wavy && Theme.waves ? thickness * 0.4 : 0
    // The middle of the line, with room for the wave.
    readonly property real radius: Math.max(0, Math.min(width, height) / 2 - thickness / 2 - thickness * 0.4)
    // The room inside the ring.
    readonly property real inner: Math.max(0, radius - thickness / 2 - amplitude)

    property real shown: Math.max(0, Math.min(value, 1))
    Behavior on shown {
        NumberAnimation {
            duration: Theme.move
            easing.type: Easing.OutCubic
        }
    }

    implicitWidth: size
    implicitHeight: size

    // The angle, from the top, where the fill ends; the gap on each side of
    // it is as wide as the line, plus the round caps.
    readonly property real end: shown * 2 * Math.PI
    readonly property real gap: radius > 0 ? thickness * 2 / radius : 0

    function at(angle: real, r: real): string {
        const x = width / 2 + r * Math.sin(angle);
        const y = height / 2 - r * Math.cos(angle);
        return `${x.toFixed(2)} ${y.toFixed(2)}`;
    }

    readonly property string fill: {
        if (end <= 0 || radius <= 0)
            return "";
        if (amplitude <= 0) {
            const sweep = Math.min(end, 2 * Math.PI - 0.001);
            return `M ${at(0, radius)} A ${radius} ${radius} 0 ${sweep > Math.PI ? 1 : 0} 1 ${at(sweep, radius)}`;
        }
        // A point about every 2 pixels along the arc.
        const steps = Math.max(8, Math.ceil(end * radius / 2));
        let path = "";
        for (let i = 0; i <= steps; i++) {
            const angle = end * i / steps;
            const r = radius + amplitude * Math.sin(angle * waves);
            path += `${i === 0 ? "M" : " L"} ${at(angle, r)}`;
        }
        return path;
    }

    readonly property string track: {
        if (radius <= 0 || end >= 2 * Math.PI - gap * 2)
            return "";
        if (end <= 0) {
            return `M ${at(0, radius)} A ${radius} ${radius} 0 1 1 ${at(Math.PI, radius)} A ${radius} ${radius} 0 1 1 ${at(0, radius)}`;
        }
        const from = end + gap;
        const to = 2 * Math.PI - gap;
        return `M ${at(from, radius)} A ${radius} ${radius} 0 ${to - from > Math.PI ? 1 : 0} 1 ${at(to, radius)}`;
    }

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: "transparent"
            strokeColor: root.trackColor
            strokeWidth: root.thickness
            capStyle: ShapePath.RoundCap

            PathSvg {
                path: root.track
            }
        }

        ShapePath {
            fillColor: "transparent"
            strokeColor: root.color
            strokeWidth: root.thickness
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin

            PathSvg {
                path: root.fill
            }
        }
    }

    Item {
        id: middle

        anchors.centerIn: parent
        width: root.inner * 2
        height: root.inner * 2
    }
}

import QtQuick
import QtQuick.Shapes

// How far along something is, from 0 to 1, as a line that waves while
// `playing`, like a track's progress, and lies flat when it stops. The
// wave drifts forward while it plays, at the theme's speed, and holds still
// without motion or out of sight. A gap and a handle mark where it is.
// With `interactive` it seeks like a thin Slider: `moved` follows the
// pointer, `released` gives the final value, and it shows the pointer's
// value until `value` catches up. Without, or disabled, it only shows,
// with no handle.
Item {
    id: root

    property real value: 0
    property bool playing: false
    property bool interactive: true
    property real thickness: 4
    property color fill: Theme.foreground
    property color trackColor: Theme.raised
    readonly property bool dragging: area.pressed
    signal moved(real value)
    signal released(real value)

    // Where the pointer put it, until the owner reports a new value.
    property real held: -1
    readonly property real shown: Math.max(0, Math.min(held >= 0 ? held : value, 1))
    onValueChanged: {
        if (!dragging)
            held = -1;
    }

    // The wave's height either side of the middle, and its length.
    readonly property real wave: thickness * 0.75
    readonly property real wavelength: thickness * 7
    property real amplitude: playing ? wave : 0
    Behavior on amplitude {
        NumberAnimation {
            duration: Theme.move
            easing.type: Easing.OutCubic
        }
    }

    // How far the wave has drifted, from 0 to one wavelength.
    property real phase: 0
    NumberAnimation {
        target: root
        property: "phase"
        from: 0
        to: root.wavelength
        duration: Theme.duration(1800)
        loops: Animation.Infinite
        running: root.visible && (root.Window.window?.visible ?? false) && root.amplitude > 0 && !Theme.reducedMotion
    }

    // Whether it seeks now: not while its owner disables it.
    readonly property bool seeks: interactive && enabled
    readonly property bool hot: seeks && (area.containsMouse || dragging)
    readonly property real handleWidth: seeks ? thickness + (hot ? 2 : 0) : 0
    readonly property real handleHeight: thickness * 3.5 + (hot ? 2 : 0)

    implicitWidth: 200
    implicitHeight: Math.max(16, thickness + wave * 2, thickness * 3.5 + 2)

    // From the first cap to the last, where the handle travels.
    readonly property real start: thickness / 2
    readonly property real reach: Math.max(0, width - thickness)
    readonly property real at: start + reach * shown
    // The space between the fill, the handle and the track.
    readonly property real gap: thickness
    readonly property real middle: height / 2

    readonly property string wavePath: {
        const to = at - handleWidth / 2 - gap - thickness / 2;
        if (to < start)
            return "";
        if (amplitude < 0.1)
            return `M ${start} ${middle} L ${to.toFixed(2)} ${middle}`;
        // A point about every 2 pixels.
        const steps = Math.max(1, Math.ceil((to - start) / 2));
        let path = "";
        for (let i = 0; i <= steps; i++) {
            const x = start + (to - start) * i / steps;
            const y = middle + amplitude * Math.sin((x - start - phase) / wavelength * 2 * Math.PI);
            path += `${i === 0 ? "M" : " L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
        }
        return path;
    }

    readonly property string trackPath: {
        const from = at + handleWidth / 2 + gap + thickness / 2;
        const to = width - thickness / 2;
        if (from > to)
            return "";
        return `M ${from.toFixed(2)} ${middle} L ${to.toFixed(2)} ${middle}`;
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
                path: root.trackPath
            }
        }

        ShapePath {
            fillColor: "transparent"
            strokeColor: root.fill
            strokeWidth: root.thickness
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin

            PathSvg {
                path: root.wavePath
            }
        }
    }

    Rectangle {
        x: root.at - width / 2
        anchors.verticalCenter: parent.verticalCenter
        visible: root.seeks
        width: root.handleWidth
        height: root.handleHeight
        radius: width / 2
        color: root.fill

        Behavior on height {
            NumberAnimation {
                duration: Theme.fast
            }
        }

        Behavior on width {
            NumberAnimation {
                duration: Theme.fast
            }
        }
    }

    MouseArea {
        id: area

        anchors.fill: parent
        enabled: root.seeks
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor

        function at(x: real): real {
            return root.reach > 0 ? Math.max(0, Math.min((x - root.start) / root.reach, 1)) : 0;
        }

        onPressed: mouse => {
            root.held = at(mouse.x);
            root.moved(root.held);
        }
        onPositionChanged: mouse => {
            if (pressed) {
                root.held = at(mouse.x);
                root.moved(root.held);
            }
        }
        onReleased: mouse => {
            root.held = at(mouse.x);
            root.released(root.held);
        }
    }
}

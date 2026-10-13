import QtQuick
import Quickshell
import QtQuick.Shapes

// How far along something is, from 0 to 1, as a line that waves while
// `playing`, like a track's progress, and lies flat when it stops. The
// wave drifts forward while it plays, at the theme's speed, and holds still
// without motion or out of sight, under a fullscreen window too, and
// moves at most 30 times a second. With `wavy` off, or the theme's waves
// off, it stays flat. A gap and a handle mark where it is.
// With `interactive` it seeks like a thin Slider: `moved` follows the
// pointer, `released` gives the final value, and it shows the pointer's
// value until `value` catches up. Without, or disabled, it only shows,
// with no handle.
Item {
    id: root

    property real value: 0
    property bool playing: false
    // Whether it may wave at all; the theme can still keep it flat.
    property bool wavy: true
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
    property real amplitude: playing && wavy && Theme.waves ? wave : 0
    Behavior on amplitude {
        NumberAnimation {
            duration: Theme.move
            easing.type: Easing.OutCubic
        }
    }

    // How far the wave has drifted, from 0 to one wavelength: a full
    // wavelength each 1.8 seconds, at 30 frames a second, while it can be
    // seen. Stopped, it holds where it was.
    Ticker {
        id: ticker

        running: root.amplitude > 0 && root.visible && Daemon.shows(root.QsWindow.window) && !Theme.reducedMotion
    }
    readonly property real period: Math.max(1, Theme.duration(1800))
    readonly property real phase: ticker.now % period / period * wavelength

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

    // Where the filled part ends, before the gap and the handle.
    readonly property real fillEnd: at - handleWidth / 2 - gap - thickness / 2
    // Where the track starts, after them.
    readonly property real trackStart: at + handleWidth / 2 + gap + thickness / 2

    // The line's height at `x`, as the wave stands now.
    function waveAt(x: real): real {
        return middle + amplitude * Math.sin((x - start - phase) / wavelength * 2 * Math.PI);
    }

    // The wave, a wavelength longer than the line could be, from its own
    // left edge. It slides by `phase` under a window that shows the filled
    // part, so it drifts without being drawn again: only its size or the
    // wave's height changing draws it again.
    readonly property string wavePath: {
        const length = width + wavelength;
        if (amplitude < 0.1)
            return `M 0 ${middle} L ${length.toFixed(2)} ${middle}`;
        // A point about every 2 pixels.
        const steps = Math.max(1, Math.ceil(length / 2));
        let path = "";
        for (let i = 0; i <= steps; i++) {
            const x = length * i / steps;
            const y = middle + amplitude * Math.sin(x / wavelength * 2 * Math.PI);
            path += `${i === 0 ? "M" : " L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
        }
        return path;
    }

    // The track, flat, after the handle.
    Rectangle {
        x: root.trackStart - root.thickness / 2
        y: root.middle - height / 2
        visible: root.trackStart <= root.width - root.thickness / 2
        width: root.width - root.trackStart + root.thickness / 2
        height: root.thickness
        radius: height / 2
        color: root.trackColor
    }

    Item {
        id: window

        x: root.start
        width: Math.max(0, root.fillEnd - root.start)
        height: root.height
        visible: root.fillEnd >= root.start
        clip: true

        Shape {
            // From a wavelength before the line's start, so the slide always
            // covers it.
            x: root.phase - root.wavelength
            width: root.width + root.wavelength
            height: root.height
            preferredRendererType: Shape.CurveRenderer

            ShapePath {
                fillColor: "transparent"
                strokeColor: root.fill
                strokeWidth: root.thickness
                capStyle: ShapePath.FlatCap
                joinStyle: ShapePath.RoundJoin

                PathSvg {
                    path: root.wavePath
                }
            }
        }
    }

    // The line's round ends, which the window cuts off, riding the wave.
    Repeater {
        model: window.visible ? [root.start, root.fillEnd] : []

        Rectangle {
            required property real modelData

            x: modelData - width / 2
            y: root.waveAt(modelData) - height / 2
            width: root.thickness
            height: root.thickness
            radius: width / 2
            color: root.fill
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

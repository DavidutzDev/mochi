import QtQuick
import QtQuick.Shapes
import qs.island

// The widget's hours look: the temperature over the next 12 hours as a
// smooth line, a column an hour, with each hour's temperature over it and
// its sky and time under it. The first column is now: a dot on the line,
// the temperature now and "Now". The line is drawn between the coldest and
// the warmest hour, at least a few degrees apart so a steady day stays
// flat. Where the columns get narrow, every other one has its labels. The
// control center's page draws it too, over 24 hours and without the age,
// which it shows itself.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""
    // How many hours it draws, and whether it says how old a stale
    // forecast is.
    property int count: 12
    property bool showAge: true
    readonly property var current: payload?.current ?? null
    readonly property var hours: (payload?.hourly ?? []).slice(0, count)
    // The running hour has the temperature now rather than the hour's.
    readonly property var temperatures: hours.map((hour, index) => index === 0 && current ? current.temperature : hour.temperature)
    readonly property real column: hours.length > 0 ? width / hours.length : width
    readonly property int step: column < 28 ? 2 : 1
    readonly property real pixel: Math.max(Theme.textCaption, Math.min(column * 0.4, height * 0.09, Theme.textHeadline))
    readonly property real icon: Math.round(Math.max(14, Math.min(column * 0.62, height * 0.15, 32)))
    // Above the line, its labels; under it, the skies and the hours.
    readonly property real above: pixel * 1.5
    readonly property real below: icon + Theme.spaceTiny + pixel * 1.4
    readonly property real dot: Math.max(8, Math.round(pixel * 0.8))
    // Where the line runs: under the labels and over the skies, with room
    // for the dot.
    readonly property real upper: above + dot / 2 + Theme.spaceTiny
    readonly property real lower: height - footer - below - dot / 2 - Theme.spaceSmall
    // While the forecast is stale, how old it is, under the hours.
    readonly property real footer: age.stale && showAge ? age.height + Theme.spaceTiny : 0
    // The degrees the line spans, at least 4.
    readonly property real coldest: Math.min(...temperatures)
    readonly property real warmest: Math.max(...temperatures)
    readonly property real span: Math.max(4, warmest - coldest)
    readonly property real ceiling: (coldest + warmest) / 2 + span / 2

    function xAt(index: int): real {
        return column * (index + 0.5);
    }

    function yAt(value: real): real {
        return upper + (ceiling - value) / span * Math.max(0, lower - upper);
    }

    // The line through every hour as cubic curves, with the slopes of a
    // monotone spline, so it never bulges past an hour colder or warmer
    // than both its neighbors.
    readonly property string line: {
        const count = temperatures.length;
        if (count < 2 || width <= 0)
            return "";
        const xs = temperatures.map((_, index) => xAt(index));
        const ys = temperatures.map(value => yAt(value));
        const slopes = [];
        for (let i = 0; i < count - 1; i++)
            slopes.push((ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]));
        const tangents = [slopes[0]];
        for (let i = 1; i < count - 1; i++)
            tangents.push(slopes[i - 1] * slopes[i] <= 0 ? 0 : 2 / (1 / slopes[i - 1] + 1 / slopes[i]));
        tangents.push(slopes[count - 2]);
        let path = `M ${xs[0].toFixed(2)} ${ys[0].toFixed(2)}`;
        for (let i = 0; i < count - 1; i++) {
            const third = (xs[i + 1] - xs[i]) / 3;
            path += ` C ${(xs[i] + third).toFixed(2)} ${(ys[i] + tangents[i] * third).toFixed(2)}`;
            path += ` ${(xs[i + 1] - third).toFixed(2)} ${(ys[i + 1] - tangents[i + 1] * third).toFixed(2)}`;
            path += ` ${xs[i + 1].toFixed(2)} ${ys[i + 1].toFixed(2)}`;
        }
        return path;
    }

    implicitWidth: 384
    implicitHeight: 160

    Missing {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.current === null
        payload: root.payload
        room: root.height
    }

    Item {
        anchors.fill: parent
        visible: root.current !== null && root.hours.length >= 2

        // The line, over a fill that fades toward the skies.
        Shape {
            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer

            ShapePath {
                strokeColor: "transparent"
                strokeWidth: 0
                fillGradient: LinearGradient {
                    x1: 0
                    y1: root.upper
                    x2: 0
                    y2: root.lower + root.dot
                    GradientStop {
                        position: 0
                        color: Qt.alpha(Theme.accent, 0.22)
                    }
                    GradientStop {
                        position: 1
                        color: Qt.alpha(Theme.accent, 0)
                    }
                }

                PathSvg {
                    path: root.line === "" ? "" : `${root.line} L ${root.xAt(root.hours.length - 1).toFixed(2)} ${(root.lower + root.dot).toFixed(2)} L ${root.xAt(0).toFixed(2)} ${(root.lower + root.dot).toFixed(2)} Z`
                }
            }

            ShapePath {
                fillColor: "transparent"
                strokeColor: Theme.accent
                strokeWidth: Math.max(2, root.pixel * 0.22)
                capStyle: ShapePath.RoundCap
                joinStyle: ShapePath.RoundJoin

                PathSvg {
                    path: root.line
                }
            }
        }

        // Now, on the line.
        Rectangle {
            x: root.xAt(0) - width / 2
            y: root.yAt(root.temperatures[0] ?? 0) - height / 2
            width: root.dot + 4
            height: width
            radius: width / 2
            color: Theme.accent
            border.width: 2
            border.color: Theme.background
        }

        Repeater {
            model: root.hours

            Item {
                id: hour

                required property var modelData
                required property int index
                readonly property bool now: index === 0
                readonly property bool labeled: index % root.step === 0

                x: root.column * index
                width: root.column
                height: root.height - root.footer

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.top: parent.top
                    visible: hour.labeled
                    text: `${Math.round(root.temperatures[hour.index] ?? 0)}°`
                    color: Theme.foreground
                    font.pixelSize: root.pixel
                    font.family: Theme.fontFamily
                    font.weight: hour.now ? Theme.weightTitle : Theme.weightBody
                }

                Symbol {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: time.top
                    anchors.bottomMargin: Theme.spaceTiny
                    visible: hour.labeled
                    name: hour.modelData.icon
                    size: root.icon
                    color: Theme.foreground
                }

                Text {
                    id: time

                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: parent.bottom
                    visible: hour.labeled
                    text: hour.now ? "Now" : String(hour.modelData.hour).padStart(2, "0")
                    color: hour.now ? Theme.foreground : Theme.muted
                    font.pixelSize: root.pixel
                    font.family: Theme.fontFamily
                    font.weight: hour.now ? Theme.weightTitle : Theme.weightBody
                }
            }
        }
    }

    Age {
        id: age

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        visible: stale && root.current !== null && root.showAge
        payload: root.payload
    }
}

import QtQuick
import qs.island

// The last readings as a line, 0 to `maximum` from bottom to top, filled
// underneath. `others` is an optional second line on the same scale, in
// `otherColor`, like a disk's writes next to its reads. A `floor` above 0
// scales the graph to its highest reading instead, never under `floor`.
Canvas {
    id: root

    property var values: []
    property var others: []
    property real maximum: 100
    property real floor: 0
    property color color: Theme.accent
    property color otherColor: Theme.success

    readonly property real ceiling: floor > 0 ? Math.max(floor, ...values, ...others) * 1.15 : maximum

    onValuesChanged: requestPaint()
    onOthersChanged: requestPaint()
    onCeilingChanged: requestPaint()
    onColorChanged: requestPaint()
    onWidthChanged: requestPaint()

    function line(context: var, values: var, color: color): void {
        if (values.length < 2)
            return;
        const step = width / 59;
        const left = width - step * (values.length - 1);
        const y = value => height - 1 - Math.min(value, ceiling) / ceiling * (height - 2);
        context.beginPath();
        context.moveTo(left, y(values[0]));
        for (let index = 1; index < values.length; index++)
            context.lineTo(left + step * index, y(values[index]));
        context.lineWidth = 1.5;
        context.strokeStyle = color;
        context.stroke();
        context.lineTo(width, height);
        context.lineTo(left, height);
        context.closePath();
        context.fillStyle = Qt.rgba(color.r, color.g, color.b, 0.15);
        context.fill();
    }

    onPaint: {
        const context = getContext("2d");
        context.reset();
        line(context, values, color);
        line(context, others, otherColor);
    }
}

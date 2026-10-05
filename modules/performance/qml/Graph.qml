import QtQuick
import qs.island

// The last readings as a line, 0 to `maximum` from bottom to top, filled
// underneath.
Canvas {
    id: root

    property var values: []
    property real maximum: 100
    property color color: Theme.accent

    onValuesChanged: requestPaint()
    onColorChanged: requestPaint()
    onWidthChanged: requestPaint()

    onPaint: {
        const context = getContext("2d");
        context.reset();
        if (values.length < 2)
            return;
        const step = width / 59;
        const left = width - step * (values.length - 1);
        const y = value => height - 1 - Math.min(value, maximum) / maximum * (height - 2);
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
}

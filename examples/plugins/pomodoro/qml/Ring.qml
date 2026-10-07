import QtQuick
import qs.island

// How much of the time is left, as a ring that empties clockwise.
Canvas {
    id: root

    property real progress: 1
    property color color: Theme.accent
    property real line: 2.5

    onProgressChanged: requestPaint()
    onColorChanged: requestPaint()

    onPaint: {
        const context = getContext("2d");
        context.reset();
        const radius = Math.min(width, height) / 2 - line;
        context.lineWidth = line;
        context.lineCap = "round";
        context.strokeStyle = Qt.alpha(Theme.muted, 0.35);
        context.beginPath();
        context.arc(width / 2, height / 2, radius, 0, Math.PI * 2);
        context.stroke();
        context.strokeStyle = root.color;
        context.beginPath();
        const start = -Math.PI / 2;
        context.arc(width / 2, height / 2, radius, start, start + Math.PI * 2 * Math.max(0, Math.min(1, progress)));
        context.stroke();
    }
}

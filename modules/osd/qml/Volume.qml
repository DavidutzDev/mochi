import QtQuick
import qs.island

// Output volume. Later steps update `payload` in place, so the bar slides
// instead of the view reloading.
Item {
    property var payload: ({})
    readonly property int percent: payload.percent ?? 0
    readonly property bool muted: payload.muted ?? false

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 12

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: {
                if (muted)
                    return "volume-muted";
                if (percent === 0)
                    return "volume-0";
                return percent < 34 ? "volume-1" : percent < 67 ? "volume-2" : "volume-3";
            }
            color: muted ? Theme.muted : Theme.foreground
        }

        ProgressBar {
            anchors.verticalCenter: parent.verticalCenter
            width: 160
            height: 6
            value: Math.min(percent, 100) / 100
            // Above 100% the bar stays full and turns to the accent color.
            fill: muted ? Theme.muted : percent > 100 ? Theme.accent : Theme.foreground
        }
    }
}

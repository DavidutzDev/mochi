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

        Icon {
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

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 160
            height: 6
            radius: 3
            color: Theme.surface

            Rectangle {
                width: parent.width * Math.min(percent, 100) / 100
                height: parent.height
                radius: parent.radius
                // Above 100% the bar stays full and turns to the accent color.
                color: muted ? Theme.muted : percent > 100 ? Theme.accent : Theme.foreground

                Behavior on width {
                    NumberAnimation {
                        duration: 150
                        easing.type: Easing.OutCubic
                    }
                }

                Behavior on color {
                    ColorAnimation {
                        duration: 150
                    }
                }
            }
        }
    }
}

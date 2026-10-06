import QtQuick
import qs.island

// Output volume. Later steps update `payload` in place, so the bar slides
// instead of the view reloading. Scrolling on it changes the volume, through
// the audio module when it runs.
Item {
    property var payload: ({})
    readonly property int percent: payload.percent ?? 0
    readonly property bool muted: payload.muted ?? false
    // The top of the bar: the audio module's max_volume, so 150% fills it
    // when the mixer goes that far.
    readonly property int maxVolume: Math.max(Daemon.state("audio")?.max_volume ?? 100, 100)

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    WheelHandler {
        // Touchpads send small steps: 5% for every notch's worth.
        property real pending: 0

        enabled: Daemon.state("audio") != null
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: event => {
            pending += event.angleDelta.y / 120;
            const notches = Math.trunc(pending);
            if (notches === 0)
                return;
            pending -= notches;
            Daemon.command("audio", "volume", ["output", `${notches > 0 ? "+" : "-"}${Math.abs(notches) * 5}`]);
        }
    }

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
            id: bar

            anchors.verticalCenter: parent.verticalCenter
            width: 160
            height: 6
            value: Math.min(percent, maxVolume) / maxVolume
            // Above 100% the bar turns to the accent color.
            fill: muted ? Theme.muted : percent > 100 ? Theme.accent : Theme.foreground

            // Where 100% is, when the bar goes further.
            Rectangle {
                visible: maxVolume > 100
                x: bar.width * 100 / maxVolume - 1
                anchors.verticalCenter: parent.verticalCenter
                width: 2
                height: bar.height + 6
                radius: 1
                color: Theme.muted
            }
        }
    }
}

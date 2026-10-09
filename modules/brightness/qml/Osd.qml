import QtQuick
import qs.island

// A display's brightness, or the keyboard's, like the volume's OSD.
// Scrolling on it changes that display by the module's step.
Item {
    id: root

    property var payload: ({})
    readonly property int percent: payload.percent ?? 0

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    WheelHandler {
        // Touchpads send small steps: one step for every notch's worth.
        property real pending: 0

        enabled: Daemon.state("brightness") != null
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: event => {
            pending += event.angleDelta.y / 120;
            const notches = Math.trunc(pending);
            if (notches === 0)
                return;
            pending -= notches;
            const step = Daemon.state("brightness")?.step ?? 5;
            Daemon.command("brightness", "set", [`${notches > 0 ? "+" : "-"}${Math.abs(notches) * step}`, root.payload.display ?? "all"]);
        }
    }

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceMedium

        // The keyboard's own symbol; a screen's sun grows with the level.
        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: root.payload.display === "keyboard" ? "keyboard" : root.percent < 34 ? "brightness_low" : root.percent < 67 ? "brightness_medium" : "brightness_high"
        }

        ProgressBar {
            anchors.verticalCenter: parent.verticalCenter
            width: 160
            height: 6
            value: root.percent / 100
            fill: Theme.foreground
        }

        // The level, as wide as "100%" so the bar doesn't shift.
        RollingText {
            id: level

            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(implicitWidth, widest.advanceWidth)
            text: `${root.percent}%`
            color: Theme.foreground
            pixelSize: Theme.textBody
            weight: Theme.weightLabel

            TextMetrics {
                id: widest

                font: level.font
                text: "100%"
            }
        }
    }
}

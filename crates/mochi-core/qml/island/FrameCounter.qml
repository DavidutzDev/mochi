import QtQuick

// Counts the frames its window draws while mochid profiles, and tells it
// each second with `frames`. Nothing runs otherwise.
Item {
    id: root

    // The monitor, and which of the shell's windows: "island" or "desktop".
    property string output: ""
    property string surface: ""
    property int frames: 0

    Connections {
        target: Daemon.profiling ? root.Window.window : null

        function onFrameSwapped(): void {
            root.frames += 1;
        }
    }

    Timer {
        interval: 1000
        repeat: true
        running: Daemon.profiling
        onRunningChanged: root.frames = 0
        onTriggered: {
            Daemon.send({
                "type": "frames",
                "output": root.output,
                "surface": root.surface,
                "frames": root.frames
            });
            root.frames = 0;
        }
    }
}

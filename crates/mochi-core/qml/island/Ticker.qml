import QtQuick

// A clock for motion that only decorates, like a wave drifting or bars
// bouncing: `now`, in milliseconds, moves on with Pulse, 30 times a second,
// and only while running. Owners run it only while they can be seen
// (Daemon.shows), so nothing redraws in a hidden panel or under a
// fullscreen window. An animation would draw a frame for each of the
// monitor's, 144 or 240 a second, for motion that reads the same at 30.
// Stopped, `now` holds where it was.
QtObject {
    id: root

    property bool running: false
    property real now: 0
    // Whether Pulse counts this one.
    property bool counted: false

    function count(): void {
        if (running === counted)
            return;
        Pulse.wanted += running ? 1 : -1;
        counted = running;
        if (running)
            now = Date.now();
    }

    onRunningChanged: count()
    Component.onCompleted: count()
    Component.onDestruction: {
        if (counted)
            Pulse.wanted -= 1;
    }

    readonly property Connections follow: Connections {
        target: root.running ? Pulse : null

        function onNowChanged(): void {
            root.now = Pulse.now;
        }
    }
}

import QtQuick

// Where playback is now. The daemon sends the position as it was at
// `read_at_ms`; this moves it on while playing.
Item {
    id: root

    property var payload: ({})
    readonly property bool playing: payload.status === "playing"
    readonly property real length: payload.length_ms ?? 0
    readonly property real position: {
        if (payload.position_ms == null)
            return 0;
        let position = payload.position_ms;
        if (playing)
            position += (now - payload.read_at_ms) * (payload.rate ?? 1);
        return Math.max(0, length > 0 ? Math.min(position, length) : position);
    }
    // From 0 to 1, or 0 for a track without a length.
    readonly property real progress: length > 0 ? position / length : 0

    property real now: Date.now()
    onPayloadChanged: now = Date.now()

    Timer {
        interval: 500
        repeat: true
        running: root.playing
        onTriggered: root.now = Date.now()
    }
}

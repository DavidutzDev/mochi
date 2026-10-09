import QtQuick

// What's left of the phase or of a custom timer right now. The daemon sends
// when it runs out, `ends_ms`; this counts down from there while it runs,
// and holds `left_ms` while paused. Seconds round up, as in the daemon's
// status, so the last one reads 0:01.
Item {
    id: root

    property var payload: ({})
    readonly property string phase: payload?.phase ?? "idle"
    readonly property bool running: phase !== "idle"
    readonly property bool paused: payload?.paused ?? false
    readonly property bool resting: phase === "break"
    // One of the custom timers, rather than the focus session.
    readonly property bool custom: phase === "timer"
    readonly property real total: payload?.total_ms ?? 0
    readonly property real remaining: {
        if (!running)
            return 0;
        if (paused || payload.ends_ms == null)
            return payload.left_ms ?? 0;
        return Math.max(0, payload.ends_ms - now);
    }
    // From 1 when it starts to 0 when it runs out.
    readonly property real progress: total > 0 ? remaining / total : 0
    // Whole minutes left, rounded up: 1 through the last minute.
    readonly property int minutes: Math.ceil(remaining / 60000)
    // m:ss, or h:mm:ss from an hour.
    readonly property string text: {
        const seconds = Math.ceil(remaining / 1000);
        const hours = Math.floor(seconds / 3600);
        const rest = String(seconds % 60).padStart(2, "0");
        const minutes = Math.floor(seconds / 60) % 60;
        return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
    }
    readonly property string what: custom ? (payload.name ?? "Timer") : resting ? "Break" : "Focus"

    property real now: Date.now()
    onPayloadChanged: now = Date.now()

    Timer {
        interval: 250
        repeat: true
        running: root.running && !root.paused && root.remaining > 0
        onTriggered: root.now = Date.now()
    }
}

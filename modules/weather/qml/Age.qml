import QtQuick
import qs.island

// How old the forecast is, for the looks to show in place of a line while
// it's stale: after the last fetch failed, or once it's older than the
// state's `stale_after`, an hour by default, as when the computer was
// offline. `stale` says whether it is.
Item {
    id: root

    property var payload: null
    property real pixelSize: Theme.textCaption
    // The wall clock in seconds, a minute at a time, for the age to grow.
    property real now: Date.now() / 1000
    readonly property real updated: payload?.updated ?? 0
    readonly property bool failed: payload?.error != null
    readonly property real staleAfter: payload?.stale_after ?? 3600
    readonly property bool stale: updated > 0 && (failed || now - updated >= staleAfter)
    readonly property string age: {
        const minutes = Math.max(0, Math.floor((now - updated) / 60));
        if (minutes < 1)
            return "just now";
        if (minutes < 60)
            return minutes === 1 ? "a minute ago" : `${minutes} minutes ago`;
        const hours = Math.floor(minutes / 60);
        return hours === 1 ? "an hour ago" : `${hours} hours ago`;
    }

    implicitWidth: mark.width + Theme.spaceTiny + full.width
    implicitHeight: Math.max(mark.height, label.implicitHeight)

    // A new state reads the clock again, so the age is right at once.
    onPayloadChanged: now = Date.now() / 1000

    TextMetrics {
        id: full

        font.pixelSize: root.pixelSize
        font.family: Theme.fontFamily
        text: `Updated ${root.age}`
    }

    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: root.now = Date.now() / 1000
    }

    Symbol {
        id: mark

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        name: root.failed ? "sync_problem" : "history"
        size: Math.round(root.pixelSize * 1.2)
        color: Theme.muted
    }

    Text {
        id: label

        anchors.left: mark.right
        anchors.leftMargin: Theme.spaceTiny
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        // Without "Updated" where the whole line doesn't fit.
        text: full.width <= root.width - mark.width - Theme.spaceTiny ? full.text : root.age
        elide: Text.ElideRight
        color: Theme.muted
        font.pixelSize: root.pixelSize
        font.family: Theme.fontFamily
    }
}

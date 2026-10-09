import QtQuick
import qs.island

// Every agent session, a mark each in the order they came, so one keeps
// its place: working, waiting for you, or done. Past four, the rest are
// counted. A click opens the list, or clears them when they're all done.
Item {
    id: root

    property var payload: ({})
    readonly property var sessions: payload.sessions ?? []
    // How many marks fit.
    readonly property int shown: 4
    readonly property bool allDone: sessions.length > 0 && sessions.every(session => session.state === "done")
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: {
        const lines = sessions.map(session => `${session.title || session.app} ${root.doing(session.state)}`);
        lines.push(allDone ? "Click to clear" : "Click for the list");
        return lines.join("\n");
    }

    function doing(status: string): string {
        return status === "waiting" ? "needs you" : status === "done" ? "is done" : "is working";
    }

    implicitWidth: Math.max(26, row.implicitWidth + Theme.spaceSmall)
    implicitHeight: 26

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 2

        Repeater {
            model: root.sessions.slice(0, root.shown)

            Glyph {
                required property var modelData

                anchors.verticalCenter: parent.verticalCenter
                status: modelData.state
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.sessions.length > root.shown
            text: `+${root.sessions.length - root.shown}`
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }
    }
}

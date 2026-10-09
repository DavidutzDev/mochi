import QtQuick
import qs.island

// Every agent session, in the order they came: what it's about, which
// agent, and what it does since when. The cross forgets one, Clear done
// the ones that finished.
Item {
    id: root

    property var payload: ({})
    readonly property var sessions: payload.sessions ?? []
    readonly property bool anyDone: sessions.some(session => session.state === "done")
    // How many fit; the rest are counted.
    property int shown: 6
    // Moves on so the times stay right while the list is open.
    property real now: Date.now()

    // "Working for 5 min", "Needs you", "Done 2 min ago".
    function doing(session: var): string {
        const minutes = session.since > 0 ? Math.floor((now - session.since) / 60000) : 0;
        const long = minutes < 60 ? `${minutes} min` : `${Math.floor(minutes / 60)} h`;
        switch (session.state) {
        case "waiting":
            return minutes < 1 ? "Needs you" : `Needs you for ${long}`;
        case "done":
            return minutes < 1 ? "Done just now" : `Done ${long} ago`;
        default:
            return minutes < 1 ? "Working" : `Working for ${long}`;
        }
    }

    implicitWidth: 400
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Timer {
        interval: 30000
        running: true
        repeat: true
        onTriggered: root.now = Date.now()
    }

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: Theme.spaceSmall

        PanelHeader {
            width: parent.width
            title: "Agents"

            ActionButton {
                visible: root.anyDone
                text: "Clear done"
                onClicked: Daemon.command("agents", "clear-done", [])
            }
        }

        Repeater {
            model: root.sessions.slice(0, root.shown)

            ListRow {
                id: entry

                required property var modelData

                width: column.width
                flat: true
                leadingSize: 24
                title: modelData.title || modelData.app
                subtitle: (modelData.title ? `${modelData.app} · ` : "") + root.doing(modelData)

                leading: Glyph {
                    anchors.centerIn: parent
                    status: entry.modelData.state
                    size: 24
                }

                trailing: IconButton {
                    icon: "close"
                    size: 15
                    onClicked: Daemon.command("agents", "clear", [entry.modelData.id])
                }
            }
        }

        Text {
            visible: root.sessions.length > root.shown
            text: `${root.sessions.length - root.shown} more`
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }
}

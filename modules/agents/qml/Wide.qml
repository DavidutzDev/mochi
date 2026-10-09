import QtQuick
import qs.island

// The wide bubble, when the user asks for it: the marks, then what the
// one session does, like "mochi-shell needs you", or how many do what.
Row {
    id: root

    property var payload: ({})
    readonly property var sessions: payload.sessions ?? []
    readonly property string summary: {
        if (sessions.length === 1) {
            const session = sessions[0];
            return `${session.title || session.app} ${bubble.doing(session.state)}`;
        }
        const count = status => sessions.filter(session => session.state === status).length;
        return [[count("waiting"), "need you"], [count("working"), "working"], [count("done"), "done"]].filter(([n]) => n > 0).map(([n, what]) => `${n} ${what}`).join(" · ");
    }

    spacing: Theme.spaceTiny

    Bubble {
        id: bubble

        anchors.verticalCenter: parent.verticalCenter
        payload: root.payload
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(implicitWidth, 200)
        text: root.summary
        elide: Text.ElideRight
        color: Theme.foreground
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
        font.weight: Theme.weightTitle
    }
}

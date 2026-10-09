import QtQuick
import qs.island

// A session that needs you, like "Claude Code needs you · mochi-shell", or
// one that finished. A click opens the list.
Item {
    id: root

    property var payload: ({})
    readonly property bool waiting: payload.state === "waiting"

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceSmall

        Glyph {
            anchors.verticalCenter: parent.verticalCenter
            status: root.payload.state ?? "done"
            size: 24
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: `${root.payload.app ?? "Agent"} ${root.waiting ? "needs you" : "is done"}`
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, 240)
            visible: text !== ""
            text: root.payload.title ?? ""
            elide: Text.ElideRight
            textFormat: Text.PlainText
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }
}

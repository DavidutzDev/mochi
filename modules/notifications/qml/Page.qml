import QtQuick
import qs.island

// The hub page: the missed notifications as rows, newest first, with do not
// disturb and clear above them. Clicking a row runs its default action; the
// cross closes it.
Item {
    id: root

    property var payload: null
    readonly property var notes: payload?.notes ?? []
    readonly property bool dnd: payload?.dnd ?? false
    // Up to five rows show; more scroll.
    readonly property int rows: Math.min(notes.length, 5)

    implicitHeight: toolbar.height + 12 + (notes.length === 0 ? 60 : rows * 60 + (rows - 1) * 8)

    Item {
        id: toolbar

        width: parent.width
        height: 32

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.notes.length === 1 ? "1 missed" : `${root.notes.length} missed`
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Row {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            Button {
                text: root.dnd ? "Do not disturb is on" : "Do not disturb"
                onClicked: Daemon.command("notifications", "dnd", ["toggle"])
            }

            Button {
                visible: root.notes.length > 0
                text: "Clear all"
                onClicked: Daemon.command("notifications", "clear", [])
            }
        }
    }

    Text {
        anchors.centerIn: list
        visible: root.notes.length === 0
        text: "Nothing missed"
        color: Theme.muted
        font.pixelSize: Theme.textSubtitle
        font.family: Theme.fontFamily
    }

    ListView {
        id: list

        anchors.top: toolbar.bottom
        anchors.topMargin: 12
        width: parent.width
        height: root.rows * 60 + Math.max(root.rows - 1, 0) * 8
        clip: true
        spacing: 8
        boundsBehavior: Flickable.StopAtBounds
        model: root.notes

        delegate: ListRow {
            id: row

            required property var modelData

            width: list.width
            height: 60
            title: modelData.summary || modelData.app
            // StyledText, so the app's name is escaped like the body was.
            subtitle: [(modelData.app ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;"), modelData.line ?? ""].filter(part => part).join(" · ")
            subtitleFormat: Text.StyledText
            onLinkActivated: link => Daemon.command("notifications", "open", [String(row.modelData.id), link])
            onClicked: {
                if (row.modelData.default)
                    Daemon.command("notifications", "invoke", [String(row.modelData.id), "default"]);
            }

            leading: AppIcon {
                note: row.modelData
                size: 34
            }

            trailing: IconButton {
                icon: "close"
                size: 14
                tone: "neutral"
                onClicked: Daemon.command("notifications", "dismiss", [String(row.modelData.id)])
            }
        }
    }
}

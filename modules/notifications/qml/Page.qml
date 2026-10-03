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

    Item {
        id: toolbar

        width: parent.width
        height: 32

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.notes.length === 1 ? "1 missed" : `${root.notes.length} missed`
            color: Theme.muted
            font.pixelSize: 13
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
        font.pixelSize: 14
    }

    ListView {
        id: list

        anchors.top: toolbar.bottom
        anchors.topMargin: 12
        anchors.bottom: parent.bottom
        width: parent.width
        clip: true
        spacing: 8
        boundsBehavior: Flickable.StopAtBounds
        model: root.notes

        delegate: Rectangle {
            id: row

            required property var modelData

            width: list.width
            height: 60
            radius: 14
            color: area.containsMouse ? Qt.lighter(Theme.surface, 1.4) : Theme.surface

            MouseArea {
                id: area

                anchors.fill: parent
                hoverEnabled: true
                cursorShape: row.modelData.default ? Qt.PointingHandCursor : Qt.ArrowCursor
                onClicked: {
                    if (row.modelData.default)
                        Daemon.command("notifications", "invoke", [String(row.modelData.id), "default"]);
                }
            }

            AppIcon {
                id: icon

                x: 14
                anchors.verticalCenter: parent.verticalCenter
                note: row.modelData
                size: 34
            }

            Column {
                anchors.left: icon.right
                anchors.leftMargin: 12
                anchors.right: close.left
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter

                Text {
                    width: parent.width
                    text: row.modelData.summary || row.modelData.app
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    color: Theme.foreground
                    font.pixelSize: 14
                    font.weight: Font.DemiBold
                }

                Text {
                    width: parent.width
                    visible: text !== ""
                    text: [row.modelData.app, (row.modelData.body ?? "").split("\n")[0]].filter(part => part).join(" · ")
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    color: Theme.muted
                    font.pixelSize: 12
                }
            }

            Button {
                id: close

                anchors.right: parent.right
                anchors.rightMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                icon: "close"
                onClicked: Daemon.command("notifications", "dismiss", [String(row.modelData.id)])
            }
        }
    }
}

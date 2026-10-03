import QtQuick
import qs.island

// The missed notifications, newest first, with do not disturb and clear.
// Clicking one runs its default action; the cross closes it.
Item {
    id: root

    property var payload: ({})
    readonly property var notes: payload.notes ?? []
    // How many fit; the rest are counted.
    property int shown: 5

    implicitWidth: 400
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: 10

        Row {
            width: parent.width
            spacing: 8

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - dnd.width - clear.width - parent.spacing * 2
                text: "Notifications"
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }

            Button {
                id: dnd

                text: root.payload.dnd ? "Do not disturb: on" : "Do not disturb: off"
                onClicked: Daemon.command("notifications", "dnd", ["toggle"])
            }

            Button {
                id: clear

                text: "Clear"
                visible: root.notes.length > 0
                onClicked: Daemon.command("notifications", "clear", [])
            }
        }

        Text {
            visible: root.notes.length === 0
            text: "Nothing missed"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Repeater {
            model: root.notes.slice(0, root.shown)

            Item {
                id: entry

                required property var modelData

                width: column.width
                height: 40

                MouseArea {
                    anchors.fill: parent
                    enabled: entry.modelData.default ?? false
                    cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: Daemon.command("notifications", "invoke", [String(entry.modelData.id), "default"])
                }

                Row {
                    anchors.fill: parent
                    spacing: 10

                    AppIcon {
                        anchors.verticalCenter: parent.verticalCenter
                        note: entry.modelData
                        size: 26
                    }

                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - 26 - close.width - parent.spacing * 2

                        Text {
                            width: parent.width
                            text: entry.modelData.summary || entry.modelData.app
                            elide: Text.ElideRight
                            textFormat: Text.PlainText
                            color: Theme.foreground
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            font.weight: Font.DemiBold
                        }

                        Text {
                            width: parent.width
                            visible: text !== ""
                            text: (entry.modelData.body ?? "").split("\n")[0]
                            elide: Text.ElideRight
                            textFormat: Text.PlainText
                            color: Theme.muted
                            font.pixelSize: Theme.textLabel
                            font.family: Theme.fontFamily
                        }
                    }

                    Button {
                        id: close

                        anchors.verticalCenter: parent.verticalCenter
                        icon: "close"
                        onClicked: Daemon.command("notifications", "dismiss", [String(entry.modelData.id)])
                    }
                }
            }
        }

        Text {
            visible: root.notes.length > root.shown
            text: `${root.notes.length - root.shown} more`
            color: Theme.muted
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
        }
    }
}

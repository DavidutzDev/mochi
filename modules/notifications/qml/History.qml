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

    EdgeLight {
        radius: Theme.radiusSurface
    }

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: Theme.spaceSmall

        PanelHeader {
            width: parent.width
            title: "Notifications"

            Button {
                text: root.payload.dnd ? "Do not disturb: on" : "Do not disturb: off"
                tone: root.payload.dnd ? "accent" : "neutral"
                onClicked: Daemon.command("notifications", "dnd", ["toggle"])
            }

            Button {
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
                height: Theme.rowHeight

                MouseArea {
                    anchors.fill: parent
                    enabled: entry.modelData.default ?? false
                    cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: Daemon.command("notifications", "invoke", [String(entry.modelData.id), "default"])
                }

                Row {
                    anchors.fill: parent
                    spacing: Theme.spaceSmall

                    AppIcon {
                        anchors.verticalCenter: parent.verticalCenter
                        note: entry.modelData
                        size: Theme.controlHeight
                    }

                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - Theme.controlHeight - close.width - parent.spacing * 2

                        Text {
                            width: parent.width
                            text: entry.modelData.summary || entry.modelData.app
                            elide: Text.ElideRight
                            textFormat: Text.PlainText
                            color: Theme.foreground
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                        }

                        Text {
                            width: parent.width
                            visible: text !== ""
                            text: entry.modelData.line ?? ""
                            elide: Text.ElideRight
                            textFormat: Text.StyledText
                            color: Theme.muted
                            linkColor: Theme.accent
                            onLinkActivated: link => Daemon.command("notifications", "open", [String(entry.modelData.id), link])
                            font.pixelSize: Theme.textCaption
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
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }
}

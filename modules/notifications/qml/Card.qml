import QtQuick
import qs.island

// The control center card: the latest missed notification, and how many there
// are.
Item {
    id: root

    property var payload: null
    readonly property var notes: payload?.notes ?? []
    readonly property var latest: notes[0] ?? null

    implicitHeight: Theme.controlHeight

    Row {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.latest === null
        spacing: Theme.spaceSmall

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: root.payload?.dnd ? "do_not_disturb_on" : "notifications_off"
            size: Theme.textTitle
            color: Theme.muted
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.payload?.dnd ? "Nothing missed, do not disturb" : "Nothing missed"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }

    Row {
        anchors.fill: parent
        visible: root.latest !== null
        spacing: Theme.spaceSmall

        AppIcon {
            id: icon

            anchors.verticalCenter: parent.verticalCenter
            note: root.latest ?? ({})
            size: Theme.controlHeight
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - icon.width - (count.visible ? count.width + Theme.spaceSmall : 0) - Theme.spaceSmall

            Text {
                width: parent.width
                text: root.latest ? root.latest.summary || root.latest.app : ""
                elide: Text.ElideRight
                textFormat: Text.PlainText
                color: Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: root.latest?.line ?? ""
                elide: Text.ElideRight
                textFormat: Text.StyledText
                color: Theme.muted
                linkColor: Theme.accent
                onLinkActivated: link => Daemon.command("notifications", "open", [String(root.latest.id), link])
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        // How many in all, when there's more than this one.
        Rectangle {
            id: count

            anchors.verticalCenter: parent.verticalCenter
            visible: root.notes.length > 1
            width: Math.max(height, number.width + Theme.spaceSmall * 2)
            height: Theme.textCaption * 2
            radius: height / 2
            color: Theme.raised

            RollingText {
                id: number

                anchors.centerIn: parent
                text: String(root.notes.length)
                color: Theme.foreground
                pixelSize: Theme.textCaption
                weight: Theme.weightTitle
            }
        }
    }
}

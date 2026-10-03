import QtQuick
import qs.island

// The hub card: the latest two missed notifications.
Item {
    id: root

    property var payload: null
    readonly property var notes: payload?.notes ?? []

    implicitHeight: notes.length === 0 ? 40 : column.implicitHeight

    Text {
        anchors.centerIn: parent
        visible: root.notes.length === 0
        text: "Nothing missed"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Column {
        id: column

        width: parent.width
        spacing: 10

        Repeater {
            model: root.notes.slice(0, 2)

            Row {
                id: entry

                required property var modelData

                width: parent.width
                spacing: 10

                AppIcon {
                    anchors.verticalCenter: parent.verticalCenter
                    note: entry.modelData
                    size: 26
                }

                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 36

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
            }
        }
    }
}

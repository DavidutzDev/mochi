import QtQuick
import qs.island

// The hub card: how many were missed, and the latest few.
Item {
    id: root

    property var payload: null
    readonly property var notes: payload?.notes ?? []

    Text {
        anchors.right: parent.right
        anchors.bottom: parent.top
        anchors.bottomMargin: 10
        visible: root.payload?.dnd ?? false
        text: "Do not disturb"
        color: Theme.muted
        font.pixelSize: 10
    }

    Text {
        anchors.centerIn: parent
        visible: root.notes.length === 0
        text: "Nothing missed"
        color: Theme.muted
        font.pixelSize: 13
    }

    Column {
        width: parent.width
        spacing: 10

        Repeater {
            model: root.notes.slice(0, 3)

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
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                    }

                    Text {
                        width: parent.width
                        visible: text !== ""
                        text: (entry.modelData.body ?? "").split("\n")[0]
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                        color: Theme.muted
                        font.pixelSize: 12
                    }
                }
            }
        }
    }
}

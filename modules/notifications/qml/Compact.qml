import QtQuick
import qs.island

// A popup, laid out like the top of the media player: a large icon or
// picture, then the app, the summary and the start of the body. Clicking it
// shows the whole notification with the app's buttons.
Item {
    id: root

    property var payload: ({})
    readonly property bool critical: payload.urgency === "critical"

    implicitWidth: 380
    implicitHeight: Math.max(icon.height, text.implicitHeight) + Theme.padding * 2

    Row {
        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: 14

        AppIcon {
            id: icon

            anchors.verticalCenter: parent.verticalCenter
            note: root.payload
            size: 64
        }

        Column {
            id: text

            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - icon.width - parent.spacing
            spacing: 2

            Text {
                width: parent.width
                visible: text !== ""
                text: root.payload.app ?? ""
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: 11
            }

            Text {
                width: parent.width
                text: root.payload.summary || root.payload.app || ""
                elide: Text.ElideRight
                textFormat: Text.PlainText
                color: root.critical ? Theme.accent : Theme.foreground
                font.pixelSize: 15
                font.weight: Font.DemiBold
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: root.payload.body ?? ""
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
                textFormat: Text.PlainText
                color: Theme.muted
                font.pixelSize: 13
            }
        }
    }
}

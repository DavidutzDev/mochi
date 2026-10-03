import QtQuick
import qs.island

// The whole notification: app and time, picture, summary, body and the
// app's actions. Clicking the text runs the default action, when there is
// one.
Item {
    id: root

    property var payload: ({})
    readonly property string id: String(payload.id ?? "")

    implicitWidth: 400
    implicitHeight: column.implicitHeight + Theme.padding * 2

    function invoke(action: string): void {
        Daemon.command("notifications", "invoke", [root.id, action]);
    }

    // "now", "5 min", "2 h", then the date.
    function ago(received: real): string {
        const minutes = Math.floor((Date.now() - received) / 60000);
        if (minutes < 1)
            return "now";
        if (minutes < 60)
            return `${minutes} min`;
        if (minutes < 24 * 60)
            return `${Math.floor(minutes / 60)} h`;
        return new Date(received).toLocaleDateString(Qt.locale(), Locale.ShortFormat);
    }

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
                width: parent.width - close.width - parent.spacing
                text: [root.payload.app, root.ago(root.payload.received_ms ?? Date.now())].filter(part => part).join(" · ")
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: 11
            }

            Button {
                id: close

                icon: "close"
                onClicked: Daemon.command("notifications", "dismiss", [root.id])
            }
        }

        Row {
            width: parent.width
            spacing: 12

            AppIcon {
                id: icon

                note: root.payload
                size: 48
            }

            Column {
                width: parent.width - icon.width - parent.spacing
                spacing: 3

                Text {
                    width: parent.width
                    text: root.payload.summary ?? ""
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    color: root.payload.urgency === "critical" ? Theme.accent : Theme.foreground
                    font.pixelSize: 15
                    font.weight: Font.DemiBold
                }

                Text {
                    width: parent.width
                    visible: text !== ""
                    text: root.payload.body ?? ""
                    wrapMode: Text.Wrap
                    maximumLineCount: 6
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    color: Theme.foreground
                    opacity: 0.85
                    font.pixelSize: 13
                }
            }

            MouseArea {
                // Over the picture and the text, not the buttons.
                x: 0
                y: 0
                width: parent.width
                height: parent.height
                enabled: root.payload.default ?? false
                cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                onClicked: root.invoke("default")
            }
        }

        Flow {
            width: parent.width
            visible: (root.payload.actions ?? []).length > 0
            spacing: 8

            Repeater {
                model: root.payload.actions ?? []

                Button {
                    required property var modelData

                    text: modelData.label
                    onClicked: root.invoke(modelData.key)
                }
            }
        }
    }
}

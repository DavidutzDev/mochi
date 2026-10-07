import QtQuick
import qs.island

// The whole notification: app and time, picture, summary, body and the
// app's actions. Clicking the text runs the default action, when there is
// one; a link in the body opens in the browser. Reply, for apps that take
// one, opens a text field that holds the keyboard: Enter sends, Escape goes
// back. An expanded notification never times out, so neither does the field.
Item {
    id: root

    property var payload: ({})
    readonly property string id: String(payload.id ?? "")
    readonly property string reply: payload.reply ?? ""
    property bool replying: false

    implicitWidth: 400
    implicitHeight: column.implicitHeight + Theme.padding * 2

    // A new message from the same app can take this one's place while the
    // field is open: the reply then goes to it, unless it takes none.
    onReplyChanged: {
        if (reply === "")
            root.closeReply();
    }

    function invoke(action: string): void {
        Daemon.command("notifications", "invoke", [root.id, action]);
    }

    function open(link: string): void {
        Daemon.command("notifications", "open", [root.id, link]);
    }

    function openReply(): void {
        root.replying = true;
        Qt.callLater(() => input.forceActiveFocus());
    }

    // Back to the buttons. The keyboard stays with the notification, so
    // the next Escape closes it.
    function closeReply(): void {
        root.replying = false;
        input.text = "";
        root.forceActiveFocus();
    }

    function send(): void {
        if (input.text.trim() !== "")
            Daemon.command("notifications", "reply", [root.id, input.text]);
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

    EdgeLight {
        radius: Theme.radiusSurface
    }

    Column {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.padding
        spacing: Theme.spaceSmall

        Row {
            width: parent.width
            spacing: Theme.spaceSmall

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - close.width - parent.spacing
                text: [root.payload.app, root.ago(root.payload.received_ms ?? Date.now())].filter(part => part).join(" · ")
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            Button {
                id: close

                icon: "close"
                onClicked: Daemon.command("notifications", "dismiss", [root.id])
            }
        }

        Item {
            width: parent.width
            height: content.height

            // Over the picture and the text, under the links in the body.
            MouseArea {
                anchors.fill: parent
                enabled: root.payload.default ?? false
                cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                onClicked: root.invoke("default")
            }

            Row {
                id: content

                width: parent.width
                spacing: Theme.spaceMedium

                AppIcon {
                    id: icon

                    note: root.payload
                    size: 48
                }

                Column {
                    width: parent.width - icon.width - parent.spacing
                    spacing: Theme.spaceTiny

                    Text {
                        width: parent.width
                        text: root.payload.summary ?? ""
                        wrapMode: Text.Wrap
                        maximumLineCount: 2
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                        color: root.payload.urgency === "critical" ? Theme.accent : Theme.foreground
                        font.pixelSize: Theme.textTitle
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightTitle
                    }

                    Text {
                        id: body

                        width: parent.width
                        visible: text !== ""
                        text: root.payload.body ?? ""
                        wrapMode: Text.Wrap
                        maximumLineCount: 6
                        elide: Text.ElideRight
                        textFormat: Text.StyledText
                        color: Theme.foreground
                        linkColor: Theme.accent
                        opacity: 0.85
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                        onLinkActivated: link => root.open(link)

                        HoverHandler {
                            cursorShape: body.hoveredLink !== "" ? Qt.PointingHandCursor : undefined
                        }
                    }
                }
            }
        }

        Flow {
            width: parent.width
            visible: !root.replying && ((root.payload.actions ?? []).length > 0 || root.reply !== "")
            spacing: Theme.spaceSmall

            Button {
                visible: root.reply !== ""
                text: root.reply
                tone: "accent"
                onClicked: root.openReply()
            }

            Repeater {
                model: root.payload.actions ?? []

                Button {
                    required property var modelData

                    text: modelData.label
                    onClicked: root.invoke(modelData.key)
                }
            }
        }

        Rectangle {
            width: parent.width
            height: 40
            visible: root.replying
            radius: height / 2
            color: Theme.surface
            border.width: input.activeFocus ? 1 : 0
            border.color: Theme.accent

            // Clicks on the field stay here instead of collapsing the
            // notification.
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.IBeamCursor
                onClicked: input.forceActiveFocus()
            }

            TextInput {
                id: input

                anchors.left: parent.left
                anchors.leftMargin: Theme.spaceLarge
                anchors.right: send.left
                anchors.rightMargin: Theme.spaceSmall
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.foreground
                selectionColor: Theme.accent
                selectedTextColor: Theme.onAccent
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                clip: true

                Keys.onReturnPressed: root.send()
                Keys.onEnterPressed: root.send()
                Keys.onEscapePressed: root.closeReply()

                Text {
                    visible: input.text === ""
                    text: "Reply"
                    color: Theme.muted
                    font: input.font
                }
            }

            Button {
                id: send

                anchors.right: parent.right
                anchors.rightMargin: Theme.spaceTiny
                anchors.verticalCenter: parent.verticalCenter
                text: "Send"
                tone: "accent"
                enabled: input.text.trim() !== ""
                onClicked: root.send()
            }
        }
    }
}

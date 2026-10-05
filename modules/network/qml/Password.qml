import QtQuick
import qs.island

// Asks for a new Wi-Fi network's password. Enter joins, Escape cancels.
Item {
    id: root

    property var payload: ({})
    readonly property string ssid: payload.ssid ?? ""

    implicitWidth: 400
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Component.onCompleted: Qt.callLater(() => input.forceActiveFocus())

    function join(): void {
        if (input.text.length > 0)
            Daemon.command("network", "password", [root.ssid, input.text]);
    }

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: 14

        Row {
            spacing: 10

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: "lock"
                size: 16
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: `Join ${root.ssid}`
                color: Theme.foreground
                font.pixelSize: Theme.textSubtitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }
        }

        Rectangle {
            width: parent.width
            height: 40
            radius: height / 2
            color: Theme.surface

            TextInput {
                id: input

                anchors.left: parent.left
                anchors.leftMargin: 16
                anchors.right: reveal.left
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                echoMode: reveal.shown ? TextInput.Normal : TextInput.Password
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                clip: true
                focus: true

                Keys.onReturnPressed: root.join()
                Keys.onEnterPressed: root.join()
                Keys.onEscapePressed: Daemon.event("dismiss")

                Text {
                    visible: input.text === ""
                    text: "Password"
                    color: Theme.muted
                    font: input.font
                }
            }

            Button {
                id: reveal

                property bool shown: false

                anchors.right: parent.right
                anchors.rightMargin: 5
                anchors.verticalCenter: parent.verticalCenter
                tone: "ghost"
                text: shown ? "Hide" : "Show"
                onClicked: shown = !shown
            }
        }

        Row {
            anchors.right: parent.right
            spacing: 8

            Button {
                text: "Cancel"
                tone: "ghost"
                onClicked: Daemon.event("dismiss")
            }

            Button {
                text: "Join"
                tone: "accent"
                enabled: input.text.length > 0
                onClicked: root.join()
            }
        }
    }
}

import QtQuick
import qs.island

// A new release, said once per version: View opens the Updates page in the
// settings, with its changelog and the update; Skip closes this.
Item {
    id: root

    property var payload: ({})

    implicitWidth: 420
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: parent.width - Theme.padding * 2
        spacing: Theme.spaceMedium

        Row {
            width: parent.width
            spacing: Theme.spaceMedium

            Rectangle {
                width: 40
                height: 40
                radius: width / 2
                color: Theme.accent

                Symbol {
                    anchors.centerIn: parent
                    name: "system_update"
                    size: Theme.textHeadline
                    color: Theme.onAccent
                }
            }

            Column {
                width: parent.width - 40 - Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    text: `Mochi ${root.payload.version ?? ""} is out`
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: `You have ${root.payload.current ?? "an older one"}. Its page in the settings says what changed and how to update.`
                    color: Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.spaceSmall

            ActionButton {
                text: "Skip"
                tone: "ghost"
                onClicked: Daemon.command("updater", "skip", [])
            }

            ActionButton {
                text: "View"
                tone: "accent"
                onClicked: Daemon.command("updater", "view", [])
            }
        }
    }
}

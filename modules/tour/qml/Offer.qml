import QtQuick
import qs.island

// The offer: the whole tour the first time Mochi starts, or what's new
// after an update. Later asks again next time; Never stops asking.
Item {
    id: root

    property var payload: ({})
    readonly property bool news: payload.tour === "new"

    implicitWidth: 440
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
                    name: root.news ? "new_releases" : "waving_hand"
                    size: Theme.textHeadline
                    color: Theme.onAccent
                }
            }

            Column {
                width: parent.width - 40 - Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    text: root.news ? `New in Mochi ${root.payload.version ?? ""}` : "Welcome to Mochi"
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.news ? `A short tour of what changed since ${root.payload.seen ?? "your last tour"}?` : "Take a two-minute tour of what it does? Your screen dims and the tour moves on by itself."
                    color: Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.spaceSmall

            Button {
                text: "Never"
                tone: "ghost"
                onClicked: Daemon.command("tour", "never", [])
            }

            Button {
                text: "Later"
                tone: "ghost"
                onClicked: Daemon.command("tour", "later", [])
            }

            Button {
                text: "Take the tour"
                icon: "play"
                tone: "accent"
                onClicked: Daemon.command("tour", "start", [])
            }
        }
    }
}

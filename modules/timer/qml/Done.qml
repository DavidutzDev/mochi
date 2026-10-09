import QtQuick
import qs.island

// A custom timer ran out: its label, how long it ran, and buttons to start
// it again, to give it a little more, or to close this.
Item {
    id: root

    property var payload: ({})
    readonly property string label: payload.label ?? ""

    function start(timer: string): void {
        Daemon.command("timer", "add", [timer]);
        Daemon.event("dismiss");
    }

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
                    name: "alarm"
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
                    text: root.label !== "" ? root.label : "Time's up"
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: `Your ${root.payload.length ?? ""} timer ran out.`
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
                text: "Done"
                tone: "ghost"
                onClicked: Daemon.event("dismiss")
            }

            ActionButton {
                text: root.payload.more_label ?? "+1 min"
                tone: "ghost"
                onClicked: root.start(root.payload.more ?? "1m")
            }

            ActionButton {
                text: "Again"
                icon: "replay"
                tone: "accent"
                onClicked: root.start(root.payload.again ?? "")
            }
        }
    }
}

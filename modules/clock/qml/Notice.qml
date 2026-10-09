import QtQuick
import qs.island
import "Time.js" as Time

// A reminder came due: what, and when it was set for, with how long ago
// when that's a while, like after a suspend. Snooze brings it up again in
// 10 minutes; Done closes it for good. Left alone, it goes after a minute
// and stays due on the clock panel.
Item {
    id: root

    property var payload: ({})
    readonly property bool twelve: payload.hours === "12"
    readonly property date at: new Date((payload.at ?? 0) * 1000)
    readonly property date now: new Date()

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
                    name: "notifications"
                    size: Theme.textHeadline
                    color: Theme.onAccent
                    filled: true
                }
            }

            Column {
                width: parent.width - 40 - Theme.spaceMedium
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    maximumLineCount: 3
                    elide: Text.ElideRight
                    text: root.payload.text ?? ""
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    text: {
                        // The tour's has no time.
                        if (root.payload.at == null)
                            return "Due now";
                        const time = Time.time(root.at, root.twelve);
                        const day = Time.sameDay(root.at, root.now) ? time : `${Time.shortDate(root.at)}, ${time}`;
                        // Late by more than a few minutes: how late.
                        if (root.now - root.at > 5 * 60000)
                            return `Reminder for ${day}, ${Time.relative(root.at, root.now)}`;
                        return `Reminder for ${day}`;
                    }
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
                text: "Snooze 10 min"
                icon: "snooze"
                tone: "ghost"
                onClicked: Daemon.command("clock", "snooze", [`${root.payload.id}`])
            }

            ActionButton {
                text: "Done"
                icon: "check"
                tone: "accent"
                onClicked: Daemon.command("clock", "done", [`${root.payload.id}`])
            }
        }
    }
}

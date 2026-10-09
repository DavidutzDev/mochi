import QtQuick
import qs.island

// A phase ran out: focus offers the break, or says it started by itself
// with `auto_break`; a break offers the next focus session. Done closes
// this.
Item {
    id: root

    property var payload: ({})
    readonly property bool focused: payload.finished === "focus"
    readonly property bool started: payload.started ?? false
    readonly property int minutes: payload.minutes ?? 0
    readonly property string sessions: {
        const done = payload.sessions ?? 0;
        return done === 1 ? "1 session done" : `${done} sessions done`;
    }
    readonly property string pause: payload.long ? `a long break of ${minutes} minutes` : `a ${minutes}-minute break`

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
                    name: root.focused ? "coffee" : "timer"
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
                    text: root.focused ? "Focus done" : "Break over"
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: {
                        if (!root.focused)
                            return `Ready for another ${root.minutes} minutes of focus?`;
                        if (root.started)
                            return `${root.sessions}, and ${root.pause} has started.`;
                        return `${root.sessions}. Take ${root.pause}?`;
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
                visible: root.focused && root.started
                text: "Skip the break"
                tone: "ghost"
                onClicked: Daemon.command("timer", "start", [])
            }

            ActionButton {
                text: "Done"
                tone: root.focused && root.started ? "accent" : "ghost"
                onClicked: Daemon.event("dismiss")
            }

            ActionButton {
                visible: !root.started
                text: root.focused ? "Start the break" : "Start focus"
                icon: root.focused ? "coffee" : "play"
                tone: "accent"
                onClicked: Daemon.command("timer", root.focused ? "break" : "start", [])
            }
        }
    }
}

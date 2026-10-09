import QtQuick
import qs.island

// At the top of the timer's page in the settings: a button that plays the
// alarm at its volume, so `volume` and `sound_file` can be tried before a
// timer runs out, and where custom timers come from.
Column {
    id: root

    property var payload: ({})
    readonly property var alarm: payload.alarm ?? null
    readonly property bool silent: alarm !== null && alarm.volume === 0

    spacing: Theme.spaceMedium

    Rectangle {
        width: root.width
        height: row.implicitHeight + Theme.spaceMedium * 2
        radius: Theme.radiusField
        color: Theme.surface

        Row {
            id: row

            x: Theme.spaceMedium
            y: Theme.spaceMedium
            width: parent.width - Theme.spaceMedium * 2
            spacing: Theme.spaceMedium

            Symbol {
                id: mark

                anchors.verticalCenter: parent.verticalCenter
                name: root.silent || root.alarm?.sound === false ? "notifications_off" : "alarm"
                size: Theme.textHeadline
                color: Theme.muted
            }

            Column {
                width: parent.width - mark.width - play.width - Theme.spaceMedium * 2
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: "The alarm"
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: {
                        if (root.alarm === null)
                            return "Turn on the timer module to hear it.";
                        if (root.silent)
                            return "The volume is 0, so it makes no sound.";
                        const when = root.alarm.sound ? (root.alarm.focus_sound ? "when a timer, a focus session or a break ends" : "when a custom timer ends") : root.alarm.focus_sound ? "when a focus session or a break ends" : "only from this button, with sound off";
                        return `Plays ${when}, at ${root.alarm.volume}%.`;
                    }
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }
            }

            ActionButton {
                id: play

                anchors.verticalCenter: parent.verticalCenter
                enabled: root.alarm !== null && !root.silent
                text: "Play it"
                icon: "play"
                onClicked: Daemon.command("timer", "test-sound", [])
            }
        }
    }

    Text {
        width: root.width
        wrapMode: Text.Wrap
        text: "Custom timers, their alarm and the launcher's :t come from mochi-clock by Xonex5, github.com/Xonex5/mochi-clock."
        color: Theme.muted
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
    }
}

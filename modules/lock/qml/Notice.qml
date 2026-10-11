import QtQuick
import qs.island

// The screen didn't lock, and why: no PAM service, a compositor without
// the session lock protocol, or another locker holding it. A click closes
// it.
Item {
    id: root

    property var payload: ({})

    implicitWidth: 420
    implicitHeight: row.implicitHeight + Theme.padding * 2

    Row {
        id: row

        x: Theme.padding
        y: Theme.padding
        width: parent.width - Theme.padding * 2
        spacing: Theme.spaceMedium

        Rectangle {
            width: 40
            height: 40
            radius: width / 2
            color: Theme.danger

            Symbol {
                anchors.centerIn: parent
                name: "lock_open"
                size: Theme.textHeadline
                color: Theme.foreground
            }
        }

        Column {
            width: parent.width - 40 - Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: "The screen isn't locked"
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: {
                    const problem = root.payload.problem ?? "";
                    return problem.charAt(0).toUpperCase() + problem.slice(1);
                }
                color: Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }
        }
    }
}

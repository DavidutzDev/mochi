import QtQuick
import qs.island

// At the top of the clock's page in the settings: the credit to
// mochi-clock, the plugin some of the clock's features come from, with a
// button that opens it on GitHub through the clock's `credit` action.
Rectangle {
    id: root

    property var payload: ({})

    implicitHeight: row.implicitHeight + Theme.spaceMedium * 2
    radius: Theme.radiusField
    color: Theme.surface

    Row {
        id: row

        x: Theme.spaceMedium
        y: Theme.spaceMedium
        width: parent.width - Theme.spaceMedium * 2
        spacing: Theme.spaceMedium

        Column {
            width: parent.width - open.width - parent.spacing
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2

            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "Inspired by mochi-clock, by Xonex5"
                color: Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "The seconds and the day's progress on Today, the stopwatch's tenths and past runs, and the list of cities come from it."
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        ActionButton {
            id: open

            anchors.verticalCenter: parent.verticalCenter
            text: "Open on GitHub"
            icon: "open_in_new"
            onClicked: Daemon.command("clock", "credit", [])
        }
    }
}

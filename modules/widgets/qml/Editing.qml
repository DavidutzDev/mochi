import QtQuick
import qs.island

// On the island while arranging widgets, on the desktop or the lock screen.
// While the drawer of widgets is closed, it shows a button for it: a click
// anywhere on the notice opens it.
Item {
    id: root

    property var payload: ({})
    readonly property var state: Daemon.state("widgets")
    readonly property bool open: state?.drawer ?? false

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceSmall

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: "dashboard_customize"
            size: Theme.textTitle
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.state?.target === "lock" ? "Arranging the lock screen" : "Arranging widgets"
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        // What a click does, as a button.
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.open
            width: add.implicitWidth + Theme.spaceMedium * 2
            height: Theme.controlHeight - Theme.spaceTiny
            radius: height / 2
            color: Theme.accent

            Row {
                id: add

                anchors.centerIn: parent
                spacing: Theme.spaceTiny

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "plus"
                    size: Theme.textBody
                    color: Theme.onAccent
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Add widgets"
                    color: Theme.onAccent
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }
            }
        }
    }
}

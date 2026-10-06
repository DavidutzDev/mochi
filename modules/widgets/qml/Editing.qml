import QtQuick
import qs.island

// On the island while arranging widgets: a click opens the drawer of
// widgets under it, or closes it.
Item {
    property var payload: ({})
    readonly property bool open: Daemon.state("widgets")?.drawer ?? false

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 10

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: "grid"
            size: 16
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Arranging widgets"
            color: Theme.foreground
            font.pixelSize: Theme.textSubtitle
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: parent.parent.open ? "click to close" : "click to add more"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: "chevron"
            rotation: parent.parent.open ? 270 : 90
            size: 12
            color: Theme.muted
        }
    }
}

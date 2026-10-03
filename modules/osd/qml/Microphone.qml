import QtQuick
import qs.island

// The default input was muted or unmuted.
Item {
    property var payload: ({})
    readonly property bool muted: payload.muted ?? false

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 10

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: muted ? "mic-muted" : "mic"
            color: muted ? Theme.accent : Theme.foreground
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: muted ? "Microphone muted" : "Microphone on"
            color: Theme.foreground
            font.pixelSize: Theme.textSubtitle
            font.family: Theme.fontFamily
        }
    }
}

import QtQuick
import qs.island

// The hub's own card: the time, large, and the date.
Item {
    id: root

    property var payload: null
    property date now: new Date()

    Timer {
        interval: 1000
        repeat: true
        running: true
        onTriggered: root.now = new Date()
    }

    Column {
        anchors.left: parent.left
        anchors.bottom: parent.bottom
        spacing: 2

        Text {
            text: root.now.toLocaleTimeString(Qt.locale(), "HH:mm")
            color: Theme.foreground
            font.pixelSize: 42
            font.weight: Font.DemiBold
            font.features: { "tnum": 1 }
        }

        Text {
            text: root.now.toLocaleDateString(Qt.locale(), "dddd d MMMM")
            color: Theme.muted
            font.pixelSize: 13
        }
    }
}

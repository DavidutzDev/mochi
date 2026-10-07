import QtQuick
import Quickshell
import qs.island

// The hub's own card: the time, large, with the weekday and the date beside
// it. Its digits roll as the minutes change.
Item {
    id: root

    property var payload: null

    implicitHeight: time.implicitHeight

    SystemClock {
        id: clock

        precision: SystemClock.Minutes
    }

    RollingText {
        id: time

        anchors.verticalCenter: parent.verticalCenter
        text: Qt.formatTime(clock.date, "HH:mm")
        pixelSize: Theme.textDisplay
        family: Theme.displayFamily
        weight: Theme.weightTitle
    }

    Column {
        anchors.left: time.right
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: parent.width
            text: clock.date.toLocaleDateString(Qt.locale(), "dddd")
            elide: Text.ElideRight
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightLabel
        }

        Text {
            width: parent.width
            text: clock.date.toLocaleDateString(Qt.locale(), "d MMMM")
            elide: Text.ElideRight
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }
}

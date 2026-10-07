import QtQuick
import qs.island

// A small battery drawn to its level, with a bolt while charging.
Item {
    id: root

    property int level: 100
    property bool charging: false
    property color color: Theme.foreground
    property real size: 18

    implicitWidth: size * 1.5
    implicitHeight: size * 0.8

    Rectangle {
        id: body

        width: parent.width - nub.width - 1
        height: parent.height
        radius: height * 0.22
        color: "transparent"
        border.color: root.color
        border.width: 1.5

        Rectangle {
            x: 3
            y: 3
            width: Math.max(0, (body.width - 6) * Math.min(root.level, 100) / 100)
            height: body.height - 6
            radius: 1.5
            color: root.color
            visible: !root.charging
        }

        Symbol {
            anchors.centerIn: parent
            visible: root.charging
            name: "bolt"
            size: body.height * 0.85
            color: root.color
        }
    }

    Rectangle {
        id: nub

        // A pixel apart from the body.
        x: body.width + 1
        anchors.verticalCenter: parent.verticalCenter
        width: 2
        height: parent.height * 0.4
        radius: 1
        color: root.color
    }
}

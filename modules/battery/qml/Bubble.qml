import QtQuick
import qs.island

// The warning while the battery is low: its level, red when critical. A
// click opens the hub.
Item {
    id: root

    property var payload: ({})
    readonly property color tint: payload.critical ? Theme.danger : Theme.accent

    implicitWidth: row.implicitWidth + 10
    implicitHeight: 26

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceTiny

        Gauge {
            anchors.verticalCenter: parent.verticalCenter
            level: root.payload.level ?? 0
            color: root.tint
            size: 13
        }

        RollingText {
            anchors.verticalCenter: parent.verticalCenter
            text: `${root.payload.level ?? 0}%`
            color: root.tint
            pixelSize: Theme.textCaption
            weight: Theme.weightTitle
        }
    }

    // Critical: hard to miss.
    SequentialAnimation on opacity {
        running: root.payload.critical ?? false
        loops: Animation.Infinite
        onRunningChanged: {
            if (!running)
                root.opacity = 1;
        }

        NumberAnimation {
            to: 0.45
            duration: 800
            easing.type: Easing.InOutSine
        }

        NumberAnimation {
            to: 1
            duration: 800
            easing.type: Easing.InOutSine
        }
    }
}

import QtQuick
import qs.island

// While a reading stays critical: its icon and value in red, breathing. A
// click opens the hub's Performance page.
Item {
    id: root

    property var payload: ({})
    readonly property var critical: payload.critical ?? []

    implicitWidth: row.implicitWidth + 10
    implicitHeight: 26

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceSmall

        Repeater {
            model: root.critical

            Row {
                required property var modelData

                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceTiny

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: modelData.icon
                    size: 14
                    color: Theme.danger
                }

                RollingText {
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.value
                    color: Theme.danger
                    pixelSize: Theme.textCaption
                    weight: Theme.weightTitle
                }
            }
        }
    }

    SequentialAnimation on opacity {
        loops: Animation.Infinite

        NumberAnimation {
            to: 0.5
            duration: 900
            easing.type: Easing.InOutSine
        }

        NumberAnimation {
            to: 1
            duration: 900
            easing.type: Easing.InOutSine
        }
    }
}

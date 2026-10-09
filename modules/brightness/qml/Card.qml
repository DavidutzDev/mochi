import QtQuick
import qs.island

// The control center's home card: a slider for the laptop's screen and one for
// each monitor that answers DDC/CI. Without any, it steps aside.
Item {
    id: root

    property var payload: null
    readonly property var displays: payload?.displays ?? []
    readonly property bool hidden: displays.length === 0

    implicitHeight: column.implicitHeight

    Column {
        id: column

        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Theme.spaceSmall

        Repeater {
            model: root.displays

            Column {
                id: entry

                required property var modelData

                width: column.width
                spacing: Theme.spaceSmall

                // Which monitor, once there are several.
                Text {
                    visible: root.displays.length > 1
                    text: entry.modelData.name
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                SliderRow {
                    width: entry.width
                    icon: entry.modelData.icon
                    value: entry.modelData.percent / 100
                    onMoved: value => Daemon.command("brightness", "set", [`${Math.round(value * 100)}`, entry.modelData.id])
                }
            }
        }
    }
}

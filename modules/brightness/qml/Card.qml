import QtQuick
import qs.island

// The control center's home card: a slider for the laptop's screen, one for
// each monitor that answers DDC/CI, and one for the keyboard's backlight.
// Without any, it steps aside.
Item {
    id: root

    property var payload: null
    readonly property var keyboard: payload?.keyboard ?? null
    readonly property var displays: payload?.displays ?? []
    readonly property var rows: keyboard ? displays.concat([keyboard]) : displays
    readonly property bool hidden: rows.length === 0

    implicitHeight: column.implicitHeight

    Column {
        id: column

        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Theme.spaceSmall

        Repeater {
            model: root.rows

            Column {
                id: entry

                required property var modelData
                // The keyboard's few levels, or none for a screen's many.
                readonly property int levels: modelData.levels ?? 0

                width: column.width
                spacing: Theme.spaceSmall

                // Which display, once there are several.
                Text {
                    visible: root.rows.length > 1
                    text: entry.modelData.name
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                SliderRow {
                    width: entry.width
                    icon: entry.modelData.icon
                    value: entry.modelData.percent / 100
                    onMoved: value => {
                        // A keyboard stops at its levels, and gets each once.
                        const percent = entry.levels > 0 ? Math.round(Math.round(value * entry.levels) * 100 / entry.levels) : Math.round(value * 100);
                        if (entry.levels > 0 && percent === entry.modelData.percent)
                            return;
                        Daemon.command("brightness", "set", [`${percent}`, entry.modelData.id]);
                    }
                }
            }
        }
    }
}

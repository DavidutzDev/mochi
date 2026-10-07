import QtQuick
import qs.island

// The mixer on the island, opened by `mochi ipc audio toggle`. Escape or a
// click outside closes it.
Item {
    id: root

    property var payload: ({})

    implicitWidth: 440
    implicitHeight: mixer.implicitHeight + Theme.padding * 2

    focus: true
    Keys.onEscapePressed: Daemon.event("dismiss")

    EdgeLight {
        radius: Theme.radiusSurface
    }

    Mixer {
        id: mixer

        x: Theme.padding
        y: Theme.padding
        width: parent.width - Theme.padding * 2
        payload: root.payload
    }
}

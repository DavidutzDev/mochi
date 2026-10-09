import QtQuick
import qs.island

// The control center's Sound page: the mixer, from the module's published
// state.
Item {
    id: root

    property var payload: null

    implicitHeight: mixer.implicitHeight

    Mixer {
        id: mixer

        width: parent.width
        payload: root.payload
        appRows: 5
    }
}

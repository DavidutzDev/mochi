import QtQuick
import Quickshell
import Quickshell.Wayland

// An invisible strip along the island's edge that keeps windows out of the
// space the idle island and the bubbles use. The island's own window
// ignores reserved space, so it always starts at the screen edge and can
// grow over the whole screen for an overlay without moving any window.
PanelWindow {
    property bool atBottom: false
    // How far windows stay from the edge.
    property int zone: 0

    anchors {
        top: !atBottom
        bottom: atBottom
        left: true
        right: true
    }

    implicitHeight: 1
    // Setting this switches the exclusion mode to Normal.
    exclusiveZone: zone
    color: "transparent"
    mask: Region {}

    WlrLayershell.namespace: "mochi-reserve"
    WlrLayershell.layer: WlrLayer.Bottom
}

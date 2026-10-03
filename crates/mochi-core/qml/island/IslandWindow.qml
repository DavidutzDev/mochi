import QtQuick
import Quickshell
import Quickshell.Wayland

// A fixed, transparent layer surface along the top of the screen. It never
// resizes: the island animates inside it, and the input mask follows the
// island so clicks anywhere else reach the windows underneath.
PanelWindow {
    anchors {
        top: true
        left: true
        right: true
    }

    implicitHeight: Theme.surfaceHeight
    color: "transparent"

    // Windows only make room for the idle island, so they don't move when it
    // grows. Setting this switches the exclusion mode to Normal.
    exclusiveZone: Theme.topMargin + Theme.idleHeight

    WlrLayershell.namespace: "mochi-island"
    WlrLayershell.layer: WlrLayer.Top

    mask: Region {
        item: island.shown ? island : null
        radius: island.radius
    }

    // Blur through ext-background-effect-v1. Compositors without the protocol
    // show the island without blur.
    BackgroundEffect.blurRegion: Region {
        item: island.shown ? island : null
        radius: island.radius
    }

    Island {
        id: island

        anchors.horizontalCenter: parent.horizontalCenter
        y: Theme.topMargin
    }
}

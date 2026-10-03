import QtQuick
import Quickshell
import Quickshell.Wayland

// A fixed, transparent layer surface along the top or bottom of the screen.
// It never resizes: the island animates inside it, and the input mask follows
// the island so clicks anywhere else reach the windows underneath.
PanelWindow {
    id: root

    anchors {
        top: !island.atBottom
        bottom: island.atBottom
        left: true
        right: true
    }

    implicitHeight: Theme.surfaceHeight
    color: "transparent"

    // Gap to the edges, which notch mode closes.
    readonly property real margin: Theme.margin * (1 - island.attached)

    // Windows only make room for the idle island, so they don't move when it
    // grows. Setting this switches the exclusion mode to Normal.
    exclusiveZone: Math.round(margin) + Theme.idleHeight

    WlrLayershell.namespace: "mochi-island"
    WlrLayershell.layer: WlrLayer.Top

    mask: IslandRegion {
        island: island
    }

    // Blur through ext-background-effect-v1. Compositors without the protocol
    // show the island without blur.
    BackgroundEffect.blurRegion: IslandRegion {
        island: island
        ears: true
    }

    Island {
        id: island

        // Sideways from the anchor's spot, then away from the edge. The
        // island grows away from the edge it sits against.
        x: {
            const away = root.margin * (1 - island.sideAttached) + Theme.offset;
            if (!island.inCorner)
                return (parent.width - width) / 2 + Theme.offset;
            return island.atRight ? parent.width - width - away : away;
        }
        y: island.atBottom ? parent.height - height - root.margin : root.margin
    }
}

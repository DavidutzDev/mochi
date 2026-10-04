import QtQuick
import Quickshell

// The island's outline as a region, for the input mask and the blur. Child
// regions are in window coordinates, like this one.
Region {
    id: root

    required property Island island
    // Include the ears: worth it for the blur, not for clicks.
    property bool ears: false
    // Pixels to stay inside the outline. The blur uses 1: the shape is drawn
    // at fractional positions with soft edges, and a blur reaching past it
    // would show as a light line around the island.
    property int inset: 0

    readonly property var outline: island.shape
    readonly property bool shown: island.shown

    x: Math.round(island.x) + inset
    y: Math.round(island.y) + inset
    width: shown ? Math.max(0, Math.round(island.width) - inset * 2) : 0
    height: shown ? Math.max(0, Math.round(island.height) - inset * 2) : 0
    topLeftRadius: Math.max(0, outline.corners.topLeft - inset)
    topRightRadius: Math.max(0, outline.corners.topRight - inset)
    bottomRightRadius: Math.max(0, outline.corners.bottomRight - inset)
    bottomLeftRadius: Math.max(0, outline.corners.bottomLeft - inset)

    // At most three ears. Each is a square with a circle cut out of it.
    EarRegion {
        region: root
        index: 0
    }
    EarRegion {
        region: root
        index: 1
    }
    EarRegion {
        region: root
        index: 2
    }

    component EarRegion: Region {
        id: square

        required property Region region
        required property int index
        readonly property var ear: region.ears && region.shown ? region.outline.ears[index] ?? null : null
        readonly property int size: ear ? Math.round(ear.size) : 0

        x: ear ? Math.round(region.island.x + ear.x) : 0
        y: ear ? Math.round(region.island.y + ear.y) : 0
        width: size
        height: size

        Region {
            shape: RegionShape.Ellipse
            intersection: Intersection.Subtract
            x: square.ear ? Math.round(square.region.island.x + square.ear.cx - square.size) : 0
            y: square.ear ? Math.round(square.region.island.y + square.ear.cy - square.size) : 0
            width: square.size * 2
            height: square.size * 2
        }
    }
}

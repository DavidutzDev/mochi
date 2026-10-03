import QtQuick
import Quickshell

// The island's outline as a region, for the input mask and the blur. Child
// regions are in window coordinates, like this one.
Region {
    id: root

    required property Island island
    // Include the ears: worth it for the blur, not for clicks.
    property bool ears: false

    readonly property var outline: island.shape
    readonly property bool shown: island.shown

    x: Math.round(island.x)
    y: Math.round(island.y)
    width: shown ? Math.round(island.width) : 0
    height: shown ? Math.round(island.height) : 0
    topLeftRadius: outline.corners.topLeft
    topRightRadius: outline.corners.topRight
    bottomRightRadius: outline.corners.bottomRight
    bottomLeftRadius: outline.corners.bottomLeft

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

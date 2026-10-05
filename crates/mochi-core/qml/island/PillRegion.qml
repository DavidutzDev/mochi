import QtQuick
import Quickshell

// A bubble pill's outline as a region, for the input mask and the blur, in
// window coordinates: a pill sits in its area's deck, or in the area itself
// like the "+N" count, and the area in the window.
Region {
    required property Item modelData
    // Pixels to stay inside the outline; see IslandRegion.
    property int inset: 0

    readonly property Item holder: modelData.parent
    readonly property bool inDeck: holder?.isDeck ?? false
    readonly property Item area: inDeck ? holder.parent : holder
    readonly property real offsetX: inDeck ? holder.x : 0
    readonly property real offsetY: inDeck ? holder.y : 0
    readonly property var corners: modelData.shape.corners
    readonly property bool shown: modelData.visible && area !== null

    x: shown ? Math.round(area.x + offsetX + modelData.x) + inset : 0
    y: shown ? Math.round(area.y + offsetY + modelData.y) + inset : 0
    width: shown ? Math.max(0, Math.round(modelData.width) - inset * 2) : 0
    height: shown ? Math.max(0, Math.round(modelData.height) - inset * 2) : 0
    topLeftRadius: Math.max(0, corners.topLeft - inset)
    topRightRadius: Math.max(0, corners.topRight - inset)
    bottomRightRadius: Math.max(0, corners.bottomRight - inset)
    bottomLeftRadius: Math.max(0, corners.bottomLeft - inset)
}

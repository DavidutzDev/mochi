import QtQuick
import Quickshell

// A bubble pill's outline as a region, for the input mask and the blur.
Region {
    required property Item modelData
    readonly property var corners: modelData.shape.corners

    item: modelData.visible ? modelData : null
    topLeftRadius: corners.topLeft
    topRightRadius: corners.topRight
    bottomRightRadius: corners.bottomRight
    bottomLeftRadius: corners.bottomLeft
}

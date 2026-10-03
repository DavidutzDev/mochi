import QtQuick
import Quickshell
import Quickshell.Wayland

// A fixed, transparent layer surface along the top or bottom of the screen,
// holding five areas from left to right: left, center-left, center,
// center-right and right. The island sits in one of them and bubbles fill
// them. The surface never resizes: everything animates inside it, and the
// input mask follows the island and the pills so clicks anywhere else reach
// the windows underneath.
PanelWindow {
    id: root

    readonly property bool atBottom: Theme.anchor === "bottom"

    // 0 floating, 1 attached to the edge in notch mode. Animated, so
    // switching modes morphs every shape.
    property real attached: Theme.mode === "notch" ? 1 : 0
    readonly property real margin: Theme.margin * (1 - attached)

    Behavior on attached {
        NumberAnimation {
            duration: 350
            easing.type: Easing.OutCubic
        }
    }

    anchors {
        top: !atBottom
        bottom: atBottom
        left: true
        right: true
    }

    implicitHeight: Theme.surfaceHeight
    color: "transparent"

    // Windows only make room for the idle island and the bubbles, so they
    // don't move when the island grows. Setting this switches the exclusion
    // mode to Normal.
    exclusiveZone: Math.round(margin) + Theme.idleHeight

    WlrLayershell.namespace: "mochi-island"
    WlrLayershell.layer: WlrLayer.Top

    // Every pill on screen, for the regions below.
    property list<Item> pills
    function addPill(pill: Item): void {
        pills = [...pills, pill];
    }
    function removePill(pill: Item): void {
        pills = pills.filter(other => other !== pill);
    }

    mask: Region {
        regions: [root.islandMask, ...pillMasks.instances]
    }

    // Blur through ext-background-effect-v1. Compositors without the protocol
    // show everything without blur.
    BackgroundEffect.blurRegion: Region {
        regions: [root.islandBlur, ...pillBlurs.instances]
    }

    property IslandRegion islandMask: IslandRegion {
        island: island
    }

    property IslandRegion islandBlur: IslandRegion {
        island: island
        ears: true
    }

    Variants {
        id: pillMasks

        model: root.pills
        PillRegion {}
    }

    Variants {
        id: pillBlurs

        model: root.pills
        PillRegion {}
    }

    readonly property real edgeY: atBottom ? height - margin : margin
    readonly property var areas: ({
            "left": left,
            "center-left": centerLeft,
            "center": center,
            "center-right": centerRight,
            "right": right
        })
    readonly property BubbleArea host: areas[Theme.islandArea] ?? center

    // Named apart from the areas' own `island` property.
    readonly property Item islandItem: island

    component Area: BubbleArea {
        island: root.islandItem
        window: root
        attached: root.attached
        atBottom: root.atBottom
        y: root.atBottom ? root.edgeY - height : root.edgeY
    }

    Area {
        id: left

        area: "left"
        x: root.margin
    }

    Area {
        id: center

        area: "center"
        x: (root.width - width) / 2
    }

    // The center-left and center-right areas hug the center area, or meet in
    // the middle of the screen when it is empty.
    Area {
        id: centerLeft

        area: "center-left"
        x: (center.width > 0 ? center.x - Theme.spacing : (root.width - Theme.spacing) / 2) - width
    }

    Area {
        id: centerRight

        area: "center-right"
        x: center.width > 0 ? center.x + center.width + Theme.spacing : (root.width + Theme.spacing) / 2
    }

    Area {
        id: right

        area: "right"
        x: root.width - width - root.margin
    }

    Island {
        id: island

        x: root.host.x + root.host.slot.x
        y: root.host.y + root.host.slot.y
        attached: root.attached
        sideAttached: root.host.onSide ? root.attached : 0
        atBottom: root.atBottom
        atRight: Theme.islandArea === "right"
    }
}

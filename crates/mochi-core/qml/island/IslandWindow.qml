import QtQuick
import Quickshell
import Quickshell.Wayland

// A fixed, transparent layer surface as tall as the screen, anchored to its
// top or bottom edge, holding five areas from left to right: left, center-left, center,
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

    // A modal activity, like the launcher, takes the keyboard and catches
    // every click so one outside the island can close it. Only the window
    // on the activity's monitor does, when it names one: two surfaces asking
    // for the keyboard would fight over it.
    readonly property var activity: Daemon.activity
    readonly property var panelOutput: activity?.output ?? activity?.payload?.output ?? null
    readonly property bool modal: (activity?.modal ?? false) && (panelOutput == null || panelOutput === screen?.name)
    // An activity with an overlay covers every monitor and asks for the
    // keyboard on each: Hyprland only sends the pointer to surfaces holding
    // the keyboard, so an overlay without it couldn't be clicked.
    readonly property bool overlaid: activity?.overlay != null
    // Any other activity that closes on a click outside catches every click
    // too, without the keyboard: the click closes it instead of reaching
    // the window underneath. Not on a monitor that isn't showing it, and not
    // after a scroll outside released it.
    readonly property bool catching: (activity?.outside ?? false) && !island.elsewhere(activity) && !releases(activity)
    // The activity whose catch a scroll released, as {id, module, key}.
    property var released: null

    // A keyed replacement, like the media notice for the next track, stays
    // released.
    function releases(activity: var): bool {
        if (!released || !activity)
            return false;
        if (activity.id === released.id)
            return true;
        return activity.key != null && activity.module === released.module && activity.key === released.key;
    }
    readonly property bool covering: modal || overlaid || catching

    // Always the whole screen: a layer surface that changes size is animated by
    // the compositor (Hyprland's `layers` animation), which would stretch a
    // frozen screenshot. Only the input mask, the layer and the keyboard change.
    implicitHeight: screen?.height ?? Theme.surfaceHeight
    color: "transparent"

    // Windows only make room for the idle island and the bubbles, so they
    // don't move when the island grows. EdgeReserve keeps that room; this
    // window ignores reserved space, so it starts at the screen edge even
    // under another bar, and covers the whole screen for an overlay.
    readonly property int zone: Math.round(margin) + Theme.idleHeight
    exclusionMode: ExclusionMode.Ignore

    WlrLayershell.namespace: "mochi-island"
    WlrLayershell.layer: modal || overlaid ? WlrLayer.Overlay : WlrLayer.Top
    WlrLayershell.keyboardFocus: modal || overlaid ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    // Every pill on screen, for the regions below.
    property list<Item> pills
    function addPill(pill: Item): void {
        pills = [...pills, pill];
    }
    function removePill(pill: Item): void {
        pills = pills.filter(other => other !== pill);
    }

    mask: covering ? everywhere : shapes

    property Region shapes: Region {
        regions: [root.islandMask, ...pillMasks.instances]
    }

    property Region everywhere: Region {
        width: root.width
        height: root.height
    }

    // Under everything: a click that misses the island closes what it
    // shows. The daemon decides how: dismissed when the user opened it or
    // it takes the keyboard, ended early otherwise.
    MouseArea {
        anchors.fill: parent
        enabled: root.covering
        acceptedButtons: Qt.AllButtons
        onClicked: Daemon.event(root.overlaid ? "dismiss" : "outside")
        // The catch takes the scroll wheel too, and a surface can't pass an
        // event on to the window underneath. A scroll outside a notice the
        // user didn't open releases the catch instead, so the next scroll
        // reaches the page; the notice stays until it times out.
        onWheel: wheel => {
            if (root.catching && !root.modal && !root.overlaid && !root.activity.expanded)
                root.released = {
                    "id": root.activity.id,
                    "module": root.activity.module,
                    "key": root.activity.key ?? null
                };
        }
    }

    // Each modal activity gets the keyboard's focus in the island, where
    // Escape closes it; after the view's own setup, so a view that takes
    // the focus for a field keeps it.
    onActivityChanged: {
        if (modal && !overlaid)
            Qt.callLater(island.takeKeys);
    }
    onModalChanged: {
        if (modal && !overlaid)
            Qt.callLater(island.takeKeys);
    }

    // The activity's overlay, under the bubbles and the island. It stays
    // loaded while activities with the same overlay follow each other, and
    // gets each one's payload.
    Loader {
        id: overlay

        anchors.fill: parent
        source: root.overlaid ? `root:/modules/${root.activity.module}/${root.activity.overlay}.qml` : ""
    }

    Binding {
        target: overlay.item
        property: "payload"
        value: root.activity?.payload
        when: overlay.item !== null && root.overlaid
    }

    Binding {
        target: overlay.item
        property: "screen"
        value: root.screen
        when: overlay.item !== null
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
        inset: 1
    }

    Variants {
        id: pillMasks

        model: root.pills
        PillRegion {}
    }

    Variants {
        id: pillBlurs

        model: root.pills
        PillRegion {
            inset: 1
        }
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
        overlay: overlay
        output: root.screen?.name ?? ""
    }
}

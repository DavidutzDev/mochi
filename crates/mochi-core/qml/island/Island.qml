import QtQuick

// The island takes the implicit size of whatever view it shows. Views never
// tell the island a size; they size themselves and the island follows with a
// spring.
//
// Two loaders take turns. A new view loads into the hidden one, the island
// springs to its size, the old view fades out and the new one fades in.
//
// Every view gets the activity's payload in a `payload` property, so views
// must declare `property var payload`.
Item {
    id: root

    // The activity on screen, or null when the daemon has nothing to show.
    property var activity: null
    readonly property bool shown: activity !== null

    // Radius of the corners away from any edge.
    readonly property real radius: Math.min(height / 2, width / 2, Theme.maxRadius)

    // How the island meets the screen edge, set by the window. In notch mode
    // it touches the edge, and also the side edge when it is the outermost
    // thing in the left or right area. Both animate, so switching modes
    // morphs the shape.
    property real attached: 0
    property real sideAttached: 0
    property bool atBottom: false
    property bool atRight: false
    readonly property alias shape: shape

    // The window's overlay loader. An activity with an overlay waits until
    // the overlay is loaded and its `ready` isn't false, so the overlay can
    // capture the screen before the island changes. An overlay that fails
    // to load doesn't hold it.
    property Loader overlay: null
    readonly property bool overlayReady: overlay !== null && (overlay.status === Loader.Error || (overlay.item !== null && overlay.item.ready !== false))

    function waiting(next: var): bool {
        return next?.overlay != null && !overlayReady;
    }

    onOverlayReadyChanged: {
        if (overlayReady && Daemon.activity?.overlay != null)
            present(Daemon.activity);
    }

    property int front: 0
    readonly property Loader frontLoader: front === 0 ? first : second
    readonly property Loader backLoader: front === 0 ? second : first

    width: frontLoader.item ? frontLoader.item.implicitWidth : Theme.idleHeight
    height: frontLoader.item ? frontLoader.item.implicitHeight : Theme.idleHeight
    opacity: shown ? 1 : 0

    Behavior on width {
        SpringAnimation {
            spring: Theme.spring
            damping: Theme.damping
            epsilon: 0.25
        }
    }

    Behavior on height {
        SpringAnimation {
            spring: Theme.spring
            damping: Theme.damping
            epsilon: 0.25
        }
    }

    Behavior on opacity {
        NumberAnimation {
            duration: Theme.fadeIn
        }
    }

    function present(next: var): void {
        const previous = activity;

        if (!next) {
            activity = null;
            return;
        }

        // The same activity, or a keyed replacement of it, in the same view:
        // only the payload changed, so update it in place without a morph.
        // A volume OSD then moves its bar instead of reloading on every step.
        if (previous && frontLoader.item && continues(previous, next)) {
            frontLoader.item.payload = next.payload;
            activity = next;
        } else if (!load(next)) {
            return;
        }

        // The daemon ignores events for activities that are gone, so a new
        // one (a keyed replacement too) has to hear that the pointer is
        // already over the island.
        if (hover.hovered && (!previous || previous.id !== next.id))
            Daemon.event("hover_enter");
    }

    // Loads the next view into the hidden slot and swaps the slots.
    function load(next: var): bool {
        // root: URLs keep views inside Quickshell's config tree. A plain file
        // path would break singletons like Theme and hot reload.
        const url = `root:/modules/${next.module}/${next.view}.qml`;
        backLoader.source = "";
        backLoader.setSource(url, { payload: next.payload });
        if (backLoader.status !== Loader.Ready) {
            console.warn(`mochi: could not load ${url}, keeping the current view`);
            backLoader.source = "";
            return false;
        }

        front = 1 - front;
        activity = next;
        return true;
    }

    function continues(previous: var, next: var): bool {
        if (previous.module !== next.module || previous.view !== next.view)
            return false;
        return previous.id === next.id || (next.key != null && next.key === previous.key);
    }

    Connections {
        target: Daemon

        function onActivityChanged(): void {
            if (!root.waiting(Daemon.activity))
                root.present(Daemon.activity);
        }
    }

    Component.onCompleted: present(Daemon.activity)

    IslandShape {
        id: shape

        anchors.fill: parent
        radius: root.radius
        attached: root.attached
        sideAttached: root.sideAttached
        earRadius: Theme.earRadius
        flipX: root.atRight
        flipY: root.atBottom
        color: Theme.background
    }

    Item {
        anchors.fill: parent
        clip: true

        IslandLoader {
            id: first
            current: root.front === 0
        }

        IslandLoader {
            id: second
            current: root.front === 1
        }
    }

    HoverHandler {
        id: hover
        onHoveredChanged: Daemon.event(hovered ? "hover_enter" : "hover_leave")
    }

    // Left click expands or collapses, or goes to the module.
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onTapped: Daemon.event("click")
    }

    // Right click closes the activity.
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: Daemon.event("dismiss")
    }
}

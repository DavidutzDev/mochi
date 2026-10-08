import QtQuick
import QtQuick.Effects

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
    // This island's monitor. An activity meant for another one doesn't show
    // here; this island shows the idle island meanwhile.
    property string output: ""

    function elsewhere(next: var): bool {
        return next?.output != null && output !== "" && next.output !== output;
    }

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
        if (overlayReady && Daemon.activity?.overlay != null && !elsewhere(Daemon.activity))
            present(Daemon.activity);
    }

    property int front: 0
    readonly property Loader frontLoader: front === 0 ? first : second
    readonly property Loader backLoader: front === 0 ? second : first

    width: frontLoader.item ? frontLoader.item.implicitWidth : Theme.idleHeight
    // No taller than `layout.surface_height`; the content clips.
    height: Math.min(frontLoader.item ? frontLoader.item.implicitHeight : Theme.idleHeight, Theme.surfaceHeight)
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
            leave(previous, null);
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
        leave(previous, next);
        if (hover.hovered && (!previous || previous.id !== next.id))
            Daemon.eventFor(next, "hover_enter");
    }

    // An activity the island stops showing while the pointer is over it,
    // like the idle clock when a notice comes, is no longer hovered: its
    // module shouldn't act on a hover that ended out of sight.
    function leave(previous: var, next: var): void {
        if (hover.hovered && previous && (!next || previous.id !== next.id))
            Daemon.eventFor(previous, "hover_leave");
    }

    // Loads the next view into the hidden slot and swaps the slots.
    function load(next: var): bool {
        // root: URLs keep views inside Quickshell's config tree. A plain file
        // path would break singletons like Theme and hot reload.
        // What the daemon shows itself, like the list of hidden bubbles,
        // comes from the "mochi" module, whose views sit here in island/.
        const url = next.module === "mochi" ? `root:/island/${next.view}.qml` : `root:/modules/${next.module}/${next.view}.qml`;
        backLoader.source = "";
        backLoader.setSource(url, {
            payload: next.payload
        });
        // A plugin's override that fails gives way to the module's own view,
        // which the daemon keeps in builtin/<module>/ next to it.
        if (backLoader.status !== Loader.Ready) {
            const builtin = `root:/builtin/${next.module}/${next.view}.qml`;
            console.warn(`mochi: could not load ${url}, trying ${builtin}`);
            backLoader.source = "";
            backLoader.setSource(builtin, {
                payload: next.payload
            });
        }
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
            root.follow();
        }
    }

    Component.onCompleted: follow()

    // Shows the daemon's activity, or the idle island when the activity is
    // meant for another monitor.
    function follow(): void {
        const next = Daemon.activity;
        if (waiting(next))
            return;
        if (!elsewhere(next))
            present(next);
        else if (Daemon.resting !== null && !elsewhere(Daemon.resting))
            present(Daemon.resting);
    }

    // A soft shadow under the shape. Theme.shadow sets its strength, and
    // transparent turns it off.
    RectangularShadow {
        anchors.fill: shape
        visible: Theme.shadow.a > 0
        radius: shape.radius
        blur: 16
        offset.y: 2
        color: Theme.shadow
    }

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
        // Files held over it: it takes them.
        border: dropZone.containsDrag ? Theme.accent : Theme.border
    }

    // The views, in a focus scope: a view that wants keys sets `focus: true`
    // on its root or takes the focus itself. Escape that the view doesn't
    // use comes up to here and closes the activity.
    FocusScope {
        id: views

        anchors.fill: parent
        clip: true
        Keys.onEscapePressed: Daemon.eventFor(root.activity, "dismiss")

        IslandLoader {
            id: first
            current: root.front === 0
            focus: current
        }

        IslandLoader {
            id: second
            current: root.front === 1
            focus: current
        }
    }

    // While the island holds the keyboard: the focus goes into the views,
    // unless a view already took it, so Escape always arrives.
    function takeKeys(): void {
        if (!views.activeFocus)
            views.forceActiveFocus();
    }

    HoverHandler {
        id: hover
        onHoveredChanged: Daemon.eventFor(root.activity, hovered ? "hover_enter" : "hover_leave")
    }

    // Files dragged over the island go to the drop module, when it runs:
    // it says where to let go, then offers what to do with them.
    DropArea {
        id: dropZone

        anchors.fill: parent
        enabled: Daemon.modules.includes("drop")
        onEntered: drag => {
            if (!drag.hasUrls) {
                drag.accepted = false;
                return;
            }
            drag.accept(Qt.CopyAction);
            Daemon.command("drop", "hover", ["on"]);
        }
        onExited: Daemon.command("drop", "hover", ["off"])
        onDropped: drop => {
            const files = drop.urls.map(url => url.toString()).filter(url => url.startsWith("file://"));
            if (files.length === 0) {
                Daemon.command("drop", "hover", ["off"]);
                return;
            }
            drop.accept(Qt.CopyAction);
            Daemon.command("drop", "files", [files.join("\n")]);
        }
    }

    // Left click expands or collapses, or goes to the module.
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onTapped: Daemon.eventFor(root.activity, "click")
    }

    // Right click closes the activity.
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: Daemon.eventFor(root.activity, "dismiss")
    }
}

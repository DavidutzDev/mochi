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

    readonly property real radius: Math.min(height / 2, Theme.maxRadius)

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

        // Same activity and view: only the payload changed, so no morph.
        if (previous && previous.id === next.id && previous.view === next.view && frontLoader.item) {
            frontLoader.item.payload = next.payload;
            activity = next;
            return;
        }

        // root: URLs keep views inside Quickshell's config tree. A plain file
        // path would break singletons like Theme and hot reload.
        const url = `root:/modules/${next.module}/${next.view}.qml`;
        backLoader.source = "";
        backLoader.setSource(url, { payload: next.payload });
        if (backLoader.status !== Loader.Ready) {
            console.warn(`mochi: could not load ${url}, keeping the current view`);
            backLoader.source = "";
            return;
        }

        front = 1 - front;
        activity = next;

        // The daemon ignores events for activities that are gone, so the new
        // one has to hear that the pointer is already over the island.
        if (hover.hovered && (!previous || previous.id !== next.id))
            Daemon.event("hover_enter");
    }

    Connections {
        target: Daemon

        function onActivityChanged(): void {
            root.present(Daemon.activity);
        }
    }

    Component.onCompleted: present(Daemon.activity)

    Rectangle {
        anchors.fill: parent
        radius: root.radius
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

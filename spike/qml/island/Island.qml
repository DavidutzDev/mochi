import QtQuick

// The island takes the implicit size of whatever view it shows. Views never
// tell the island a size; they size themselves and the island follows with a
// spring.
//
// Two loaders take turns. A new activity loads into the hidden one, the
// island springs to the new view's size, the old view fades out and the new
// one fades in.
Item {
    id: root

    readonly property real radius: Math.min(height / 2, Theme.maxRadius)

    property int front: 0
    readonly property Loader frontLoader: front === 0 ? first : second
    readonly property Loader backLoader: front === 0 ? second : first

    width: frontLoader.item ? frontLoader.item.implicitWidth : Theme.idleHeight
    height: frontLoader.item ? frontLoader.item.implicitHeight : Theme.idleHeight

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

    function show(activity: var): void {
        // root: URLs resolve inside Quickshell's config tree. A plain file path
        // would load the view outside it, where singletons like Theme break.
        const url = `root:/modules/${activity.module}/${activity.view}.qml`;

        // Clear first so presenting the same view again builds a fresh item.
        backLoader.source = "";
        backLoader.setSource(url, { payload: activity.payload });

        if (backLoader.status !== Loader.Ready) {
            console.warn(`mochi: could not load ${url}, keeping the current view`);
            backLoader.source = "";
            return;
        }

        front = 1 - front;
    }

    Connections {
        target: Daemon

        function onActivityChanged(): void {
            if (Daemon.activity)
                root.show(Daemon.activity);
        }
    }

    Component.onCompleted: {
        if (Daemon.activity)
            show(Daemon.activity);
    }

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
        onHoveredChanged: Daemon.event(hovered ? "hover_enter" : "hover_leave")
    }

    TapHandler {
        onTapped: Daemon.event("click")
    }
}

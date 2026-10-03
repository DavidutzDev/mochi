import QtQuick
import "Lists.js" as Lists

// One bubble on the edge, or a group of them sharing one background. Small
// views make it a circle, or a capsule for a group; wide views make it a
// pill. It has the island's shape, so in notch mode it hangs from the edge
// with ears too.
//
// Bubble views size themselves, declare `property var payload`, and get new
// payloads in place. Small ones fit in about 26 pixels.
Item {
    id: root

    // [{id, module, key, view, payload}], left to right.
    required property var bubbles
    property real attached: 0
    property real sideAttached: 0
    property bool atBottom: false
    property bool atRight: false
    readonly property alias shape: shape

    readonly property var byKey: {
        const map = {};
        for (const bubble of bubbles)
            map[Lists.bubbleKey(bubble)] = bubble;
        return map;
    }

    readonly property bool wide: bubbles.some(bubble => bubble.wide)
    // Small views are about 26 pixels, centered in the circle; wide ones get
    // the island's padding.
    readonly property real inset: wide ? Theme.padding : Math.max(4, (implicitHeight - 26) / 2)

    implicitWidth: Math.max(implicitHeight, row.implicitWidth + inset * 2)
    implicitHeight: Theme.idleHeight
    width: implicitWidth
    height: implicitHeight

    Behavior on width {
        SpringAnimation {
            spring: Theme.spring
            damping: Theme.damping
            epsilon: 0.25
        }
    }

    onBubblesChanged: Lists.sync(views, bubbles.map(Lists.bubbleKey))

    // Grows in when it first appears.
    scale: 0.6
    opacity: 0
    Component.onCompleted: {
        Lists.sync(views, bubbles.map(Lists.bubbleKey));
        scale = 1;
        opacity = 1;
    }

    Behavior on scale {
        NumberAnimation {
            duration: Theme.fadeIn
            easing.type: Easing.OutBack
        }
    }

    Behavior on opacity {
        NumberAnimation {
            duration: Theme.fadeIn
        }
    }

    IslandShape {
        id: shape

        anchors.fill: parent
        radius: Math.min(height / 2, width / 2, Theme.maxRadius)
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

        Row {
            id: row

            anchors.centerIn: parent
            spacing: root.wide ? 12 : 4

            Repeater {
                model: ListModel {
                    id: views
                }

                Item {
                    id: slot

                    required property string key
                    readonly property var bubble: root.byKey[key] ?? null

                    width: loader.item ? loader.item.implicitWidth : 0
                    height: root.height

                    // Under the view, so buttons inside the view get their
                    // own clicks.
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (slot.bubble)
                                Daemon.bubbleClick(slot.bubble.id);
                        }
                    }

                    Loader {
                        id: loader

                        // Changes only when the view does: payloads go in
                        // place through the binding below.
                        readonly property string url: slot.bubble ? `root:/modules/${slot.bubble.module}/${slot.bubble.view}.qml` : ""

                        anchors.verticalCenter: parent.verticalCenter
                        onUrlChanged: {
                            if (url === "")
                                return;
                            setSource(url, { payload: slot.bubble.payload });
                            if (status !== Loader.Ready)
                                console.warn(`mochi: could not load ${url}`);
                        }
                    }

                    Binding {
                        target: loader.item
                        property: "payload"
                        value: slot.bubble?.payload
                        when: loader.item !== null && slot.bubble !== null
                    }
                }
            }
        }
    }
}

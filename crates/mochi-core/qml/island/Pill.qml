import QtQuick
import QtQuick.Effects
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
    // Set when it should go: it shrinks and fades, then says `gone`. Set
    // back before then, it grows again.
    property bool exiting: false
    readonly property alias shape: shape

    signal gone

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

    // Grows in when it first appears, and shrinks away when it goes.
    scale: 0.6
    opacity: 0
    Component.onCompleted: {
        Lists.sync(views, bubbles.map(Lists.bubbleKey));
        scale = 1;
        opacity = 1;
    }
    onExitingChanged: {
        scale = exiting ? 0.6 : 1;
        opacity = exiting ? 0 : 1;
        if (exiting)
            leave.restart();
        else
            leave.stop();
    }

    Timer {
        id: leave

        interval: Theme.fadeIn
        onTriggered: root.gone()
    }

    Behavior on scale {
        NumberAnimation {
            duration: Theme.fadeIn
            easing.type: root.exiting ? Easing.InBack : Easing.OutBack
        }
    }

    Behavior on opacity {
        NumberAnimation {
            duration: Theme.fadeIn
        }
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
        radius: Math.min(height / 2, width / 2, Theme.maxRadius)
        attached: root.attached
        sideAttached: root.sideAttached
        earRadius: Theme.earRadius
        flipX: root.atRight
        flipY: root.atBottom
        color: Theme.background
        border: Theme.border
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
                    // The bubble left the group: the view keeps its last
                    // payload while it fades and its room closes.
                    required property bool leaving
                    readonly property var bubble: root.byKey[key] ?? null
                    // The last payload, which a leaving view keeps.
                    property var payload: bubble?.payload
                    onBubbleChanged: {
                        if (bubble)
                            payload = bubble.payload;
                    }

                    width: !leaving && loader.item ? loader.item.implicitWidth : 0
                    height: root.height
                    opacity: leaving ? 0 : 1
                    onLeavingChanged: {
                        if (leaving)
                            closing.restart();
                        else
                            closing.stop();
                    }

                    // No overshoot: it would make the width negative on the
                    // way to 0.
                    Behavior on width {
                        NumberAnimation {
                            duration: Theme.move
                            easing.type: Easing.OutCubic
                        }
                    }

                    Behavior on opacity {
                        NumberAnimation {
                            duration: Theme.fadeIn
                        }
                    }

                    Timer {
                        id: closing

                        interval: Theme.move
                        onTriggered: Lists.removeLeft(views, slot.key)
                    }

                    // Under the view, so buttons inside the view get their
                    // own clicks. It covers half the gap on each side, so no
                    // spot in a group is dead.
                    // Sees the pointer over the view too, whatever handles
                    // its clicks.
                    HoverHandler {
                        id: pointer
                    }

                    MouseArea {
                        anchors.fill: parent
                        enabled: !slot.leaving
                        anchors.leftMargin: -row.spacing / 2
                        anchors.rightMargin: -row.spacing / 2
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

                        anchors.centerIn: parent
                        onUrlChanged: {
                            if (url === "")
                                return;
                            setSource(url, {
                                payload: slot.bubble.payload
                            });
                            // A failed override gives way to the module's
                            // own view, as in the island.
                            if (status !== Loader.Ready) {
                                const builtin = `root:/builtin/${slot.bubble.module}/${slot.bubble.view}.qml`;
                                console.warn(`mochi: could not load ${url}, trying ${builtin}`);
                                setSource(builtin, {
                                    payload: slot.bubble.payload
                                });
                            }
                            if (status !== Loader.Ready)
                                console.warn(`mochi: could not load ${url}`);
                        }
                    }

                    Binding {
                        target: loader.item
                        property: "payload"
                        value: slot.payload
                        when: loader.item !== null
                    }

                    // More about it, once the pointer rests on it: the
                    // view's own `tooltip`, or else its module's.
                    BubbleTip {
                        target: slot
                        hovered: pointer.hovered && !slot.leaving
                        text: loader.item?.tooltip ?? slot.bubble?.tooltip ?? ""
                    }
                }
            }
        }
    }
}

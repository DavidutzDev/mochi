import QtQuick

// A line of light along the top edge of a panel, card or tile. At rest it's
// a faint hairline; while hovered, a brighter band leans toward the pointer;
// while `working`, the band sweeps back and forth; and `flash()` brightens
// it briefly, when something finishes. It takes no clicks.
Item {
    id: root

    // The corners it keeps clear of, usually its owner's radius.
    property real radius: Theme.radiusSurface
    property bool working: false
    property color color: Theme.accent

    // Where the band sits, from 0 at the left to 1 at the right.
    property real position: 0.5
    property real sweep: 0
    property real flashing: 0

    readonly property real lit: Math.max(flashing, working ? 0.9 : hover.hovered ? 0.55 : 0)

    function flash(): void {
        glow.restart();
    }

    anchors.left: parent ? parent.left : undefined
    anchors.right: parent ? parent.right : undefined
    anchors.top: parent ? parent.top : undefined
    height: 2

    HoverHandler {
        id: hover

        // On the owner, so its whole surface counts, not this thin line.
        parent: root.parent
        onPointChanged: {
            if (hovered && root.parent)
                root.position = Math.max(0, Math.min(1, point.position.x / Math.max(1, root.parent.width)));
        }
        onHoveredChanged: {
            if (!hovered)
                root.position = 0.5;
        }
    }

    Behavior on position {
        enabled: !root.working

        SmoothedAnimation {
            duration: Theme.move
            velocity: -1
        }
    }

    SequentialAnimation on sweep {
        running: root.working && root.visible
        loops: Animation.Infinite

        NumberAnimation {
            from: 0.15
            to: 0.85
            duration: 1300
            easing.type: Easing.InOutSine
        }

        NumberAnimation {
            from: 0.85
            to: 0.15
            duration: 1300
            easing.type: Easing.InOutSine
        }
    }

    SequentialAnimation {
        id: glow

        NumberAnimation {
            target: root
            property: "flashing"
            to: 1
            duration: Theme.fast
            easing.type: Easing.OutCubic
        }

        NumberAnimation {
            target: root
            property: "flashing"
            to: 0
            duration: Theme.move * 2
            easing.type: Easing.OutQuint
        }
    }

    // The hairline, there all the time.
    Rectangle {
        x: root.radius
        width: Math.max(0, root.width - root.radius * 2)
        height: 1
        gradient: Gradient {
            orientation: Gradient.Horizontal

            GradientStop {
                position: 0
                color: "transparent"
            }

            GradientStop {
                position: 0.5
                color: Qt.alpha(Theme.foreground, 0.16)
            }

            GradientStop {
                position: 1
                color: "transparent"
            }
        }
    }

    // The band.
    Rectangle {
        readonly property real room: Math.max(0, root.width - root.radius * 2 - width)

        x: root.radius + room * (root.working ? root.sweep : root.position)
        width: Math.max(0, (root.width - root.radius * 2) * 0.4)
        height: 1.5
        opacity: root.lit
        gradient: Gradient {
            orientation: Gradient.Horizontal

            GradientStop {
                position: 0
                color: "transparent"
            }

            GradientStop {
                position: 0.5
                color: root.color
            }

            GradientStop {
                position: 1
                color: "transparent"
            }
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.move
                easing.type: Easing.OutQuint
            }
        }
    }
}

import QtQuick

// A line along the top edge of a panel, card or tile that says something
// happens: while `working`, a band sweeps back and forth, and `flash()`
// shows it briefly when something finishes. At rest, and under the
// pointer, there's nothing: a glow on every edge reads as decoration. It
// takes no clicks.
Item {
    id: root

    // The corners it keeps clear of, usually its owner's radius.
    property real radius: Theme.radiusSurface
    property bool working: false
    // Neutral unless the owner means something by it, like a color picked.
    property color color: Qt.alpha(Theme.foreground, 0.6)

    property real sweep: 0
    property real flashing: 0

    readonly property real lit: Math.max(flashing, working ? 0.9 : 0)

    function flash(): void {
        glow.restart();
    }

    anchors.left: parent ? parent.left : undefined
    anchors.right: parent ? parent.right : undefined
    anchors.top: parent ? parent.top : undefined
    height: 2

    SequentialAnimation on sweep {
        running: root.working && root.visible
        loops: Animation.Infinite

        NumberAnimation {
            from: 0.15
            to: 0.85
            duration: Theme.duration(1300)
            easing.type: Easing.InOutSine
        }

        NumberAnimation {
            from: 0.85
            to: 0.15
            duration: Theme.duration(1300)
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

    // The band.
    Rectangle {
        readonly property real room: Math.max(0, root.width - root.radius * 2 - width)

        // Centered for a flash.
        x: root.radius + room * (root.working ? root.sweep : 0.5)
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

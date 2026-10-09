import QtQuick
import qs.island

// The clock widget's analog look: a round face with a tick for each hour,
// and one for each minute when there's room, the hour and minute hands,
// and with `seconds`, a seconds hand in the accent color that steps round.
// Here or in another time zone, like the other looks.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool seconds: settings.seconds === true
    readonly property real side: Math.min(width, height)
    // Minute ticks only where they're far enough apart to tell.
    readonly property bool minuteTicks: side >= 120

    ClockTime {
        id: time

        payload: root.payload
        zone: root.settings.timezone ?? ""
        seconds: root.seconds
    }

    readonly property var parts: time.parts
    readonly property real minuteAngle: (parts.minutes + (seconds ? parts.seconds / 60 : 0)) * 6
    readonly property real hourAngle: (parts.hours % 12 + parts.minutes / 60) * 30

    Item {
        id: face

        anchors.centerIn: parent
        width: root.side
        height: root.side

        Repeater {
            model: root.minuteTicks ? 60 : 12

            // A tick, turned about the face's middle.
            Item {
                id: tick

                required property int index
                readonly property bool hour: !root.minuteTicks || index % 5 === 0

                anchors.fill: parent
                rotation: index * (root.minuteTicks ? 6 : 30)

                Rectangle {
                    x: (parent.width - width) / 2
                    y: root.side * 0.03
                    width: tick.hour ? Math.max(2, root.side * 0.022) : Math.max(1, root.side * 0.008)
                    height: tick.hour ? root.side * 0.07 : root.side * 0.03
                    radius: width / 2
                    color: tick.hour ? Theme.foreground : Theme.muted
                }
            }
        }

        Hand {
            angle: root.hourAngle
            length: root.side * 0.26
            thickness: Math.max(3, root.side * 0.05)
            color: Theme.foreground
        }

        Hand {
            angle: root.minuteAngle
            length: root.side * 0.38
            thickness: Math.max(2, root.side * 0.032)
            color: Theme.foreground
        }

        Hand {
            visible: root.seconds
            angle: root.parts.seconds * 6
            length: root.side * 0.4
            tail: root.side * 0.1
            thickness: Math.max(1.5, root.side * 0.012)
            color: Theme.accent
            // Steps round with each second, clockwise past the top too.
            animated: true
        }

        Rectangle {
            anchors.centerIn: parent
            width: Math.max(5, root.side * 0.05)
            height: width
            radius: width / 2
            color: root.seconds ? Theme.accent : Theme.foreground
        }
    }

    // A hand from the face's middle, turned `angle` degrees from twelve.
    component Hand: Item {
        id: hand

        property real angle: 0
        property real length: 10
        // How far it reaches past the middle, on the other side.
        property real tail: 0
        property real thickness: 2
        property color color: Theme.foreground
        property bool animated: false

        anchors.fill: parent
        rotation: angle

        Behavior on rotation {
            enabled: hand.animated
            RotationAnimation {
                direction: RotationAnimation.Clockwise
                duration: Theme.fast
                easing.type: Easing.OutCubic
            }
        }

        Rectangle {
            x: (parent.width - width) / 2
            y: parent.height / 2 - hand.length
            width: hand.thickness
            height: hand.length + hand.tail
            radius: width / 2
            color: hand.color
        }
    }
}

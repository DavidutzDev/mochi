import QtQuick

// Picks a value from 0 to 1 by clicking or dragging. Thick, with an icon
// inside, like a control center's volume; or thin, like a seek bar, with
// `thickness` and no icon. `moved` follows the pointer, `released` gives the
// final value; the slider shows the pointer's value until `value` catches up.
// With `reset` set, a double click goes back to it, like a volume to 100%.
Item {
    id: root

    property real value: 0
    property string icon: ""
    property real thickness: 32
    property color fill: Theme.foreground
    // Where a double click puts it, from 0 to 1; below 0, nowhere.
    property real reset: -1
    readonly property bool dragging: area.pressed
    signal moved(real value)
    signal released(real value)

    // Where the pointer put it, until the owner reports a new value.
    property real held: -1
    readonly property real shown: Math.max(0, Math.min(held >= 0 ? held : value, 1))
    onValueChanged: {
        if (!dragging)
            held = -1;
    }

    implicitWidth: 200
    implicitHeight: Math.max(thickness, 16)

    Rectangle {
        id: track

        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        height: root.thickness + (root.thickness < 12 && (area.containsMouse || root.dragging) ? 2 : 0)
        radius: height / 2
        color: Theme.raised

        Rectangle {
            width: Math.max(track.height, track.width * root.shown)
            height: track.height
            radius: track.radius
            color: root.fill
            visible: root.shown > 0
        }

        Symbol {
            x: (track.height - size) / 2
            anchors.verticalCenter: parent.verticalCenter
            visible: root.icon !== ""
            name: root.icon
            size: Math.min(18, track.height - 12)
            // Dark on the fill, muted on the empty track.
            color: track.width * root.shown > track.height ? Theme.background : Theme.muted
        }
    }

    MouseArea {
        id: area

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor

        function at(x: real): real {
            return Math.max(0, Math.min(x / width, 1));
        }

        onPressed: mouse => {
            root.held = at(mouse.x);
            root.moved(root.held);
        }
        onPositionChanged: mouse => {
            if (pressed) {
                root.held = at(mouse.x);
                root.moved(root.held);
            }
        }
        // Set by a double click, which comes before the second release: that
        // release then leaves the reset alone.
        property bool resetting: false

        onReleased: mouse => {
            if (resetting) {
                resetting = false;
                return;
            }
            root.held = at(mouse.x);
            root.released(root.held);
        }
        onDoubleClicked: {
            if (root.reset < 0)
                return;
            resetting = true;
            root.held = root.reset;
            root.released(root.reset);
        }
    }
}

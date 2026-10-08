import QtQuick
import QtQuick.Window
import qs.island

// Under the island on every monitor while the tour runs: the screen dims,
// and this layer takes every click and key, so nothing behind it reacts.
// At the bottom, what the step shows and how far the tour is. Space and →
// go on, ← goes back, Escape asks whether to stop, and Enter stops.
Item {
    id: root

    property var payload: ({})
    property var screen
    readonly property var step: payload?.step ?? null
    readonly property bool confirming: payload?.confirming ?? false
    readonly property int index: payload?.index ?? 0
    readonly property int count: Math.max(payload?.count ?? 1, 1)

    // Every monitor's layer asks for the keyboard; the compositor gives it
    // to one. The island swapping its view for each step takes the focus
    // with it, so the layer takes it back whenever it moves.
    readonly property Item focused: Window.activeFocusItem
    onFocusedChanged: takeKeys()
    function takeKeys(): void {
        if (!keys.activeFocus)
            Qt.callLater(() => keys.forceActiveFocus());
    }

    // Every click and scroll stops here.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        hoverEnabled: true
        onWheel: wheel => wheel.accepted = true
    }

    Rectangle {
        id: shade

        anchors.fill: parent
        color: "black" // design: a dim layer is black at an opacity
        opacity: 0
        Component.onCompleted: opacity = 0.55

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.move * 2
                easing.type: Easing.OutCubic
            }
        }
    }

    Item {
        id: keys

        focus: true
        Component.onCompleted: root.takeKeys()

        Keys.onPressed: event => {
            event.accepted = true;
            if (root.confirming) {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                    Daemon.command("tour", "end", []);
                else if (event.key === Qt.Key_Escape)
                    Daemon.command("tour", "continue", []);
                return;
            }
            switch (event.key) {
            case Qt.Key_Escape:
                Daemon.command("tour", "stop", []);
                break;
            case Qt.Key_Space:
            case Qt.Key_Right:
            case Qt.Key_Return:
            case Qt.Key_Enter:
                Daemon.command("tour", "next", []);
                break;
            case Qt.Key_Left:
            case Qt.Key_Backspace:
                Daemon.command("tour", "back", []);
                break;
            }
        }
    }

    // The step's caption, or the question whether to stop. It rises in
    // when the tour starts, and its text slides in with each step.
    Rectangle {
        id: panel

        property real rise: Theme.spaceHuge * 2
        opacity: 0
        transform: Translate {
            y: Theme.anchor === "bottom" ? -panel.rise : panel.rise
        }
        Component.onCompleted: {
            opacity = 1;
            rise = 0;
        }

        Behavior on rise {
            NumberAnimation {
                duration: Theme.move * 1.5
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Theme.overshoot
            }
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.move
            }
        }

        Behavior on height {
            NumberAnimation {
                duration: Theme.move
                easing.type: Easing.OutCubic
            }
        }

        // Across the screen from the island, so it never covers it.
        anchors.horizontalCenter: parent.horizontalCenter
        y: Theme.anchor === "bottom" ? Theme.spaceHuge * 2 : parent.height - height - Theme.spaceHuge * 2
        width: Math.min(640, parent.width - Theme.spaceHuge * 2)
        height: column.implicitHeight + Theme.padding * 2
        radius: Theme.radiusSurface
        color: Theme.background
        border.width: 1
        border.color: Theme.border

        Column {
            id: column

            x: Theme.padding
            y: Theme.padding
            width: parent.width - Theme.padding * 2
            spacing: Theme.spaceSmall

            // Each step's text slides in from the side it comes from.
            property real shift: 0
            property int last: root.index
            transform: Translate {
                x: column.shift
            }

            Connections {
                target: root

                function onIndexChanged(): void {
                    column.shift = root.index > column.last ? Theme.spaceHuge : -Theme.spaceHuge;
                    column.last = root.index;
                    column.opacity = 0;
                    entering.restart();
                }
            }

            ParallelAnimation {
                id: entering

                NumberAnimation {
                    target: column
                    property: "shift"
                    to: 0
                    duration: Theme.move
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: Theme.overshoot
                }

                NumberAnimation {
                    target: column
                    property: "opacity"
                    to: 1
                    duration: Theme.fadeIn
                }
            }

            Row {
                visible: !root.confirming
                width: parent.width
                spacing: Theme.spaceSmall

                Text {
                    text: (root.payload?.chapter ?? "").toUpperCase()
                    color: Theme.accent
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                    font.letterSpacing: 1
                }

                Text {
                    text: `${root.index + 1} / ${root.count}`
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }
            }

            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: root.confirming ? "Stop the tour?" : root.step?.title ?? ""
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: root.confirming ? "It starts again here next time, from Settings › Tour or \"tour\" in the launcher." : root.step?.caption ?? ""
                color: Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                lineHeight: 1.15
            }

            // How far the step and the tour are.
            Item {
                visible: !root.confirming
                width: parent.width
                height: 4

                Rectangle {
                    width: parent.width
                    height: parent.height
                    radius: height / 2
                    color: Theme.raised
                }

                Rectangle {
                    id: progress

                    height: parent.height
                    radius: height / 2
                    color: Theme.accent
                    readonly property real done: root.index / root.count
                    width: parent.width * (done + timer.fraction / root.count)
                }

                // The step's own time, filling its share of the bar.
                NumberAnimation {
                    id: timer

                    property real fraction: 0
                    target: timer
                    property: "fraction"
                    from: 0
                    to: 1
                    duration: (root.payload?.seconds ?? 6) * 1000
                    running: !root.confirming
                }

                Connections {
                    target: root

                    function onIndexChanged(): void {
                        timer.restart();
                    }

                    function onConfirmingChanged(): void {
                        if (!root.confirming)
                            timer.restart();
                    }
                }
            }

            Item {
                width: parent.width
                height: Theme.controlHeight

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.confirming ? "Enter stops · Esc carries on" : "Space next · ← back · Esc stop"
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                Row {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.spaceSmall

                    Button {
                        visible: root.confirming
                        text: "Carry on"
                        tone: "ghost"
                        onClicked: Daemon.command("tour", "continue", [])
                    }

                    Button {
                        visible: root.confirming
                        text: "Stop"
                        tone: "danger"
                        onClicked: Daemon.command("tour", "end", [])
                    }

                    Button {
                        visible: !root.confirming
                        icon: "chevron"
                        rotation: 180
                        enabled: root.index > 0
                        onClicked: Daemon.command("tour", "back", [])
                    }

                    Button {
                        visible: !root.confirming
                        text: root.index + 1 >= root.count ? "Done" : "Next"
                        tone: "accent"
                        onClicked: Daemon.command("tour", "next", [])
                    }
                }
            }
        }
    }
}

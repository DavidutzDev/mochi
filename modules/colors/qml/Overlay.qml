import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.island

// The picker over one monitor: the screen frozen the moment it opened, and
// a magnifier at the pointer with the pixels around it, the one under the
// pointer outlined, and its hex. A click picks it; Escape or a right click
// cancels.
//
// What's drawn here is scaled to logical pixels, so it isn't where the
// colors come from. The module copied every screen as the picker opened;
// the overlay sends the pointer's place in the screen's own pixels and
// gets the pixels around it back, and a click picks from that copy.
Item {
    id: root

    property var payload: ({})
    property var screen: null

    readonly property string output: screen?.name ?? ""
    readonly property bool covering: screen !== null && height >= screen.height
    readonly property bool ready: covering && frozen.hasContent

    // The screen in its own pixels, as the module copied it.
    readonly property var copied: payload?.screens?.[output] ?? null
    readonly property real ratioX: copied && width > 0 ? copied.width / width : 1
    readonly property real ratioY: copied && height > 0 ? copied.height / height : 1
    readonly property int cells: payload?.cells ?? 11
    readonly property int cell: 12
    // What the module sent for this screen's pointer, or null.
    readonly property var lens: payload?.lens?.output === output && pointer.containsMouse ? payload.lens : null

    // The pixel under the pointer, in the screen's own pixels.
    function pixel(): point {
        const x = Math.min(Math.floor(pointer.mouseX * ratioX), (copied?.width ?? 1) - 1);
        const y = Math.min(Math.floor(pointer.mouseY * ratioY), (copied?.height ?? 1) - 1);
        return Qt.point(Math.max(0, x), Math.max(0, y));
    }

    function send(action: string): void {
        const at = pixel();
        Daemon.command("colors", action, [output, String(at.x), String(at.y)]);
    }

    function cancel(): void {
        Daemon.command("colors", "cancel", []);
    }

    // At most one hover a frame: the module answers each with the pixels.
    property point asked: Qt.point(-1, -1)
    Timer {
        id: hover

        interval: 16
        onTriggered: {
            const at = root.pixel();
            if (at.x === root.asked.x && at.y === root.asked.y)
                return;
            root.asked = at;
            root.send("hover");
        }
    }

    // Every monitor's overlay asks for the keyboard; the compositor gives it
    // to one, and the island swapping its view takes it back, so it's taken
    // again whenever it moves.
    readonly property Item focused: Window.activeFocusItem
    onFocusedChanged: takeKeys()
    Component.onCompleted: takeKeys()
    function takeKeys(): void {
        if (!keys.activeFocus)
            Qt.callLater(() => keys.forceActiveFocus());
    }

    ScreencopyView {
        id: frozen

        // The monitor's size, never the window's, so it can't stretch.
        width: root.screen?.width ?? 0
        height: root.screen?.height ?? 0
        visible: root.covering
        captureSource: root.screen
        live: false
    }

    MouseArea {
        id: pointer

        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.CrossCursor

        onPositionChanged: {
            if (!hover.running)
                hover.start();
        }
        onContainsMouseChanged: {
            if (containsMouse)
                hover.start();
        }
        onClicked: mouse => {
            if (mouse.button === Qt.RightButton)
                root.cancel();
            else
                root.send("select");
        }
    }

    // The magnifier, below and right of the pointer, or wherever it fits.
    Item {
        id: magnifier

        readonly property real gap: 24

        visible: root.lens !== null
        width: root.cells * root.cell + 4
        height: width + label.height + 8
        x: pointer.mouseX + gap + width > root.width ? pointer.mouseX - gap - width : pointer.mouseX + gap
        y: pointer.mouseY + gap + height > root.height ? pointer.mouseY - gap - height : pointer.mouseY + gap

        Rectangle {
            id: lens

            width: parent.width
            height: width
            radius: Theme.radiusSmall
            color: Theme.background
            border.color: Theme.foreground
            border.width: 2

            Grid {
                anchors.centerIn: parent
                columns: root.cells

                Repeater {
                    model: root.cells * root.cells

                    Rectangle {
                        required property int index
                        readonly property string pixel: root.lens?.cells?.[index] ?? ""

                        width: root.cell
                        height: root.cell
                        color: pixel === "" ? "transparent" : pixel
                        border.color: "#1affffff"
                        border.width: 1
                    }
                }
            }

            // The pixel picked, outlined in black and white so it shows on
            // any color.
            Rectangle {
                anchors.centerIn: parent
                width: root.cell + 2
                height: width
                color: "transparent"
                border.color: "white"
                border.width: 1

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: -1
                    color: "transparent"
                    border.color: "black"
                    border.width: 1
                }
            }
        }

        Rectangle {
            id: label

            anchors.top: lens.bottom
            anchors.topMargin: 8
            anchors.horizontalCenter: lens.horizontalCenter
            width: row.implicitWidth + 20
            height: 28
            radius: height / 2
            color: Theme.background

            Row {
                id: row

                anchors.centerIn: parent
                spacing: 8

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 14
                    height: 14
                    radius: 7
                    color: root.lens?.swatch ?? "transparent"
                    border.color: Theme.border
                    border.width: 1
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.lens?.hex ?? ""
                    color: Theme.foreground
                    font.pixelSize: Theme.textLabel
                    font.family: Theme.fontFamily
                    font.features: {
                        "tnum": 1
                    }
                }
            }
        }
    }

    Item {
        id: keys

        focus: true
        Keys.onEscapePressed: root.cancel()
        Keys.onReturnPressed: {
            if (pointer.containsMouse)
                root.send("select");
        }
        Keys.onEnterPressed: {
            if (pointer.containsMouse)
                root.send("select");
        }
    }
}

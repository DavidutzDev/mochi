import QtQuick
import qs.island

// A monitor's workspaces as dots, the active one stretched into a pill, with
// its name. Clicking a dot switches to that workspace, and the scroll wheel
// to the previous or next one. A new switch updates the payload in place, so
// the pill slides between dots.
Item {
    id: root

    property var payload: ({})
    readonly property var workspaces: payload.workspaces ?? []

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    // Scrolled distance not yet a whole notch, so a touchpad's small steps
    // add up to one switch instead of many.
    property real scrolled: 0

    function step(by: int): void {
        const active = workspaces.findIndex(workspace => workspace.active);
        const next = workspaces[Math.max(0, Math.min(workspaces.length - 1, active + by))];
        if (next && !next.active)
            Daemon.command("workspaces", "switch", [payload.output, next.name]);
    }

    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: event => {
            root.scrolled += event.angleDelta.y;
            while (Math.abs(root.scrolled) >= 120) {
                const up = root.scrolled > 0;
                root.scrolled -= up ? 120 : -120;
                root.step(up ? -1 : 1);
            }
        }
    }

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 12

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.payload.label != null
            text: root.payload.label ?? ""
            color: Theme.muted
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
        }

        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6

            Repeater {
                // A count rather than the array, so a payload update keeps
                // the same dots and their animations run.
                model: root.workspaces.length

                Rectangle {
                    id: dot

                    required property int index
                    readonly property var workspace: root.workspaces[index] ?? ({})

                    anchors.verticalCenter: parent.verticalCenter
                    width: workspace.active ? 22 : 8
                    height: 8
                    radius: 4
                    color: workspace.active ? Theme.foreground : workspace.urgent ? Theme.accent : Theme.muted

                    Behavior on width {
                        NumberAnimation {
                            duration: Theme.move
                            easing.type: Easing.BezierSpline
                            easing.bezierCurve: Theme.overshoot
                        }
                    }

                    Behavior on color {
                        ColorAnimation {
                            duration: 200
                        }
                    }

                    MouseArea {
                        // A target larger than the 8px dot.
                        anchors.fill: parent
                        anchors.margins: -5
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Daemon.command("workspaces", "switch", [root.payload.output, dot.workspace.name])
                    }
                }
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.payload.active ?? ""
            color: Theme.foreground
            font.pixelSize: Theme.textSubtitle
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }
    }
}

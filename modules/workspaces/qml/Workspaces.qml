import QtQuick
import qs.island

// A monitor's workspaces as dots, the active one stretched into a pill, with
// its name. Clicking a dot switches to that workspace. A new switch updates
// the payload in place, so the pill slides between dots.
Item {
    id: root

    property var payload: ({})
    readonly property var workspaces: payload.workspaces ?? []

    implicitWidth: row.implicitWidth + Theme.padding * 2
    implicitHeight: 40

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 12

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.payload.label != null
            text: root.payload.label ?? ""
            color: Theme.muted
            font.pixelSize: 12
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
                            duration: 200
                            easing.type: Easing.OutCubic
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
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }
    }
}

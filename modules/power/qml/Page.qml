import QtQuick
import qs.island

// The hub page: one tile per power action, then the power profiles. Tiles
// that end the session ask first: the first click turns "Shut down" into
// "Shut down?", a second click within a few seconds acts. Acting closes the
// hub.
Item {
    id: root

    property var payload: null
    readonly property var buttons: payload?.buttons ?? []
    readonly property var profiles: payload?.profiles ?? []
    // The action waiting for its second click, or "".
    property string armed: ""

    readonly property var profileLooks: ({
        "power-saver": { "label": "Power saver", "icon": "leaf" },
        "balanced": { "label": "Balanced", "icon": "scale" },
        "performance": { "label": "Performance", "icon": "bolt" }
    })

    Timer {
        id: disarm

        interval: 3000
        onTriggered: root.armed = ""
    }

    function press(button: var): void {
        if (button.confirm && armed !== button.action) {
            armed = button.action;
            disarm.restart();
            return;
        }
        armed = "";
        Daemon.command("power", button.action, []);
        Daemon.event("dismiss");
    }

    Column {
        width: parent.width
        spacing: 12

        Text {
            text: "Session"
            color: Theme.muted
            font.pixelSize: 12
            font.weight: Font.DemiBold
        }

        Row {
            id: tiles

            width: parent.width
            spacing: 10

            Repeater {
                model: root.buttons

                Rectangle {
                    id: tile

                    required property var modelData
                    readonly property bool asking: root.armed === modelData.action

                    width: (tiles.width - tiles.spacing * (root.buttons.length - 1)) / Math.max(root.buttons.length, 1)
                    height: 112
                    radius: 16
                    color: asking ? Theme.accent : area.containsMouse ? Qt.lighter(Theme.surface, 1.5) : Theme.surface

                    Behavior on color {
                        ColorAnimation {
                            duration: 150
                        }
                    }

                    Column {
                        anchors.centerIn: parent
                        spacing: 10

                        Symbol {
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: tile.modelData.icon
                            size: 26
                            color: tile.asking ? Theme.background : Theme.foreground
                        }

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: tile.asking ? `${tile.modelData.label}?` : tile.modelData.label
                            color: tile.asking ? Theme.background : Theme.foreground
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.press(tile.modelData)
                    }
                }
            }
        }

        Item {
            width: 1
            height: 8
        }

        Text {
            visible: root.profiles.length > 0
            text: "Power profile"
            color: Theme.muted
            font.pixelSize: 12
            font.weight: Font.DemiBold
        }

        // Like the navbar: the active profile is a white pill.
        Rectangle {
            id: segments

            visible: root.profiles.length > 0
            width: parent.width
            height: 52
            radius: height / 2
            color: Theme.surface

            readonly property real segment: (width - 8) / Math.max(root.profiles.length, 1)
            readonly property int active: root.profiles.indexOf(root.payload?.profile ?? "")

            Rectangle {
                visible: segments.active >= 0
                x: 4 + segments.segment * segments.active
                y: 4
                width: segments.segment
                height: parent.height - 8
                radius: height / 2
                color: Theme.foreground

                Behavior on x {
                    NumberAnimation {
                        duration: 220
                        easing.type: Easing.OutCubic
                    }
                }
            }

            Row {
                x: 4
                y: 4

                Repeater {
                    model: root.profiles

                    Item {
                        id: choice

                        required property string modelData
                        required property int index
                        readonly property bool selected: index === segments.active
                        readonly property var look: root.profileLooks[modelData] ?? { "label": modelData, "icon": "" }

                        width: segments.segment
                        height: segments.height - 8

                        Row {
                            anchors.centerIn: parent
                            spacing: 8

                            Symbol {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: choice.look.icon !== ""
                                name: choice.look.icon
                                size: 18
                                color: choice.selected ? Theme.background : Theme.muted
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: choice.look.label
                                color: choice.selected ? Theme.background : Theme.foreground
                                font.pixelSize: 13
                                font.weight: Font.DemiBold
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Daemon.command("power", "profile", [choice.modelData])
                        }
                    }
                }
            }
        }
    }
}

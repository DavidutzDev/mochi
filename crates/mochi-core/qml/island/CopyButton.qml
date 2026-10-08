import QtQuick

// "Copy", and an arrow with the other formats: runs `module`'s `action`
// with the format, like `widgets copy toml`. The first format is what the
// button itself copies. The menu opens above, or below with `down`, and
// the button says "Copied" for a moment after.
Item {
    id: root

    required property string module
    property string action: "copy"
    // [{label, format}], the button's own first.
    property var formats: [
        {
            "label": "Copy as TOML",
            "format": "toml"
        },
        {
            "label": "Copy as Nix",
            "format": "nix"
        }
    ]
    property bool down: false
    property bool open: false
    signal copied(string format)

    implicitWidth: label.implicitWidth + Theme.spaceHuge + arrow.width + 1
    implicitHeight: 30

    function copy(format: string): void {
        Daemon.command(module, action, [format]);
        open = false;
        done.restart();
        copied(format);
    }

    Timer {
        id: done

        interval: 1500
    }

    // One pill, Copy and the arrow its two halves, each lit on hover.
    component Half: Item {
        id: half

        property bool first: true
        property alias hovered: area.containsMouse
        default property alias content: inner.data
        signal clicked

        height: parent ? parent.height : 0

        // Rounded on the outer side only: a pill, and a square over its
        // inner corners.
        Item {
            anchors.fill: parent
            visible: area.containsMouse

            Rectangle {
                anchors.fill: parent
                radius: height / 2
                color: Theme.highlight
            }

            Rectangle {
                x: half.first ? parent.width / 2 : 0
                width: parent.width / 2
                height: parent.height
                color: Theme.highlight
            }
        }

        Item {
            id: inner

            anchors.fill: parent
        }

        MouseArea {
            id: area

            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: half.clicked()
        }
    }

    Rectangle {
        id: pill

        anchors.fill: parent
        radius: height / 2
        color: Theme.raised

        Half {
            id: main

            width: parent.width - arrow.width - 1
            onClicked: root.copy(root.formats[0].format)

            Row {
                id: label

                anchors.centerIn: parent
                spacing: Theme.spaceSmall

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: done.running ? "check" : "copy"
                    size: 15
                    color: Theme.foreground
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: done.running ? "Copied" : "Copy"
                    color: Theme.foreground
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.weight: Font.DemiBold
                }
            }
        }

        Rectangle {
            x: main.width
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: parent.height - Theme.spaceSmall * 2
            color: Theme.highlight
        }

        Half {
            id: arrow

            x: main.width + 1
            width: root.height + Theme.spaceTiny
            first: false
            onClicked: root.open = !root.open

            Symbol {
                anchors.centerIn: parent
                name: "chevron"
                size: 12
                rotation: root.open ? 270 : 90
                color: Theme.foreground
            }
        }
    }

    Rectangle {
        id: menu

        visible: root.open
        x: root.width - width
        y: root.down ? root.height + Theme.spaceTiny : -height - Theme.spaceTiny
        z: 10
        width: Math.max(root.width, 150)
        height: formats.implicitHeight + Theme.spaceTiny * 2
        radius: Theme.radiusField
        color: Theme.raised

        Column {
            id: formats

            x: Theme.spaceTiny
            y: Theme.spaceTiny
            width: parent.width - Theme.spaceTiny * 2

            Repeater {
                model: root.formats

                Rectangle {
                    id: format

                    required property var modelData

                    width: formats.width
                    height: Theme.controlHeight
                    radius: Theme.radiusControl
                    color: area.containsMouse ? Theme.highlight : "transparent"

                    Text {
                        x: Theme.spaceMedium
                        anchors.verticalCenter: parent.verticalCenter
                        text: format.modelData.label
                        color: Theme.foreground
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.copy(format.modelData.format)
                    }
                }
            }
        }
    }
}

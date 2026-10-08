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

    implicitWidth: buttons.implicitWidth
    implicitHeight: buttons.implicitHeight

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

    Row {
        id: buttons

        spacing: 2

        Button {
            width: Math.max(implicitWidth, root.width - more.width - buttons.spacing)
            text: done.running ? "Copied" : "Copy"
            icon: done.running ? "check" : "copy"
            onClicked: root.copy(root.formats[0].format)
        }

        Button {
            id: more

            icon: "chevron"
            iconSize: 12
            rotation: root.open ? 270 : 90
            onClicked: root.open = !root.open
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

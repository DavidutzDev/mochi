import QtQuick
import qs.island

// A color just picked, and copied in the default format: a big swatch and
// a row for each format. Clicking a row copies it.
Item {
    id: root

    property var payload: ({})
    // The format copied last, for the check mark.
    property string copied: ""

    implicitWidth: 400
    implicitHeight: row.implicitHeight + Theme.padding * 2

    Timer {
        id: forget

        interval: 1500
        onTriggered: root.copied = ""
    }

    Row {
        id: row

        x: Theme.padding
        y: Theme.padding
        width: parent.width - Theme.padding * 2
        spacing: 14

        Rectangle {
            id: swatch

            width: 104
            height: formats.implicitHeight
            radius: Theme.radiusMedium
            color: root.payload.swatch ?? "transparent"
            border.color: Theme.border
            border.width: 1
        }

        Column {
            id: formats

            width: parent.width - swatch.width - parent.spacing
            spacing: 2

            Text {
                width: parent.width
                bottomPadding: 4
                text: `Copied ${root.payload.text ?? ""}`
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }

            Repeater {
                model: root.payload.formats ?? []

                Rectangle {
                    id: line

                    required property var modelData

                    width: formats.width
                    height: 26
                    radius: Theme.radiusSmall
                    color: area.containsMouse ? Theme.raised : "transparent"

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.fast
                        }
                    }

                    Text {
                        id: label

                        x: 8
                        width: 50
                        anchors.verticalCenter: parent.verticalCenter
                        text: line.modelData.label
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.weight: Font.DemiBold
                    }

                    Text {
                        anchors.left: label.right
                        anchors.right: mark.left
                        anchors.rightMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        text: line.modelData.text
                        elide: Text.ElideRight
                        color: Theme.foreground
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }

                    Symbol {
                        id: mark

                        anchors.right: parent.right
                        anchors.rightMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        visible: area.containsMouse || root.copied === line.modelData.format
                        name: root.copied === line.modelData.format ? "check" : "copy"
                        size: 14
                        color: root.copied === line.modelData.format ? Theme.success : Theme.muted
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            Daemon.command("colors", "copy", [root.payload.color, line.modelData.format]);
                            root.copied = line.modelData.format;
                            forget.restart();
                        }
                    }
                }
            }
        }
    }
}

import QtQuick
import qs.island

// The desktop widget: the latest colors as swatches, as many as the widget
// fits, newest first, with a line over them. Clicking a swatch copies it in
// the default format; hovering shows its text there. The button beside the
// line opens the picker.
Item {
    id: root

    property var payload: null
    // What the desktop gives a widget; this one has no settings.
    property var settings: ({})
    property string instance: ""

    readonly property var history: payload?.history ?? []
    readonly property int gap: Theme.spaceSmall
    // Swatches are at least this big, and stretch to fill each row.
    readonly property int least: Theme.controlHeight
    readonly property int columns: Math.max(1, Math.floor((swatches.width + gap) / (least + gap)))
    readonly property real size: (swatches.width - gap * (columns - 1)) / columns
    readonly property int rows: Math.max(1, Math.floor((swatches.height + gap) / (size + gap)))
    readonly property var colors: history.slice(0, columns * rows)
    property var hovered: null
    // The text of the color just copied, for a moment.
    property string copied: ""

    Timer {
        id: forget

        interval: 1500
        onTriggered: root.copied = ""
    }

    Item {
        id: header

        width: parent.width
        height: pick.height

        Symbol {
            id: mark

            anchors.verticalCenter: parent.verticalCenter
            visible: root.copied !== ""
            name: "check"
            size: Theme.textCaption
            color: Theme.success
        }

        Text {
            anchors.left: mark.visible ? mark.right : parent.left
            anchors.leftMargin: mark.visible ? Theme.spaceTiny : 0
            anchors.right: pick.left
            anchors.rightMargin: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            text: {
                if (root.copied !== "")
                    return `Copied ${root.copied}`;
                if (root.hovered)
                    return root.hovered.text;
                return "Recent colors";
            }
            elide: Text.ElideRight
            color: root.hovered || root.copied !== "" ? Theme.foreground : Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
            font.weight: Theme.weightLabel
        }

        IconButton {
            id: pick

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            icon: "colorize"
            size: 14
            tone: "neutral"
            onClicked: Daemon.command("colors", "pick", [])
        }
    }

    Item {
        id: swatches

        anchors.top: header.bottom
        anchors.topMargin: Theme.spaceSmall
        anchors.bottom: parent.bottom
        width: parent.width

        Text {
            anchors.fill: parent
            visible: root.history.length === 0
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            wrapMode: Text.Wrap
            text: "Colors you pick show here"
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Grid {
            columns: root.columns
            spacing: root.gap

            Repeater {
                model: root.colors

                Rectangle {
                    id: swatch

                    required property var modelData

                    width: root.size
                    height: root.size
                    radius: Theme.radiusControl
                    color: modelData.swatch
                    border.color: area.containsMouse ? Theme.foreground : Theme.border
                    border.width: area.containsMouse ? 2 : 1
                    scale: area.pressed ? 0.9 : 1

                    Behavior on scale {
                        NumberAnimation {
                            duration: Theme.fast
                        }
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onContainsMouseChanged: {
                            if (containsMouse)
                                root.hovered = swatch.modelData;
                            else if (root.hovered === swatch.modelData)
                                root.hovered = null;
                        }
                        onClicked: {
                            Daemon.command("colors", "copy", [swatch.modelData.color]);
                            root.copied = swatch.modelData.text;
                            forget.restart();
                        }
                    }
                }
            }
        }
    }
}

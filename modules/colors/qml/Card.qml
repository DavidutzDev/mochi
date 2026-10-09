import QtQuick
import qs.island

// The control center card: the latest colors as dots, as many as fit, with a
// line under them and a button to pick one. Clicking a dot copies it in the
// default format; hovering shows its text.
Item {
    id: root

    property var payload: null
    readonly property int dot: Theme.textTitle + Theme.spaceTiny
    // As many dots as fit beside the button.
    readonly property int fits: Math.max(1, Math.floor((width - pick.width - Theme.spaceSmall + Theme.spaceTiny) / (dot + Theme.spaceTiny)))
    readonly property var colors: (payload?.history ?? []).slice(0, fits)
    property var hovered: null
    property bool copied: false

    implicitHeight: Theme.rowHeight

    Timer {
        id: forget

        interval: 1500
        onTriggered: root.copied = false
    }

    Row {
        id: dots

        anchors.left: parent.left
        anchors.bottom: parent.verticalCenter
        spacing: Theme.spaceTiny

        Repeater {
            model: root.colors

            Rectangle {
                id: dot

                required property var modelData

                width: root.dot
                height: root.dot
                radius: height / 2
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
                            root.hovered = dot.modelData;
                        else if (root.hovered === dot.modelData)
                            root.hovered = null;
                    }
                    onClicked: {
                        Daemon.command("colors", "copy", [dot.modelData.color]);
                        root.copied = true;
                        forget.restart();
                    }
                }
            }
        }
    }

    Text {
        anchors.left: parent.left
        anchors.right: pick.left
        anchors.rightMargin: Theme.spaceSmall
        anchors.top: root.colors.length > 0 ? parent.verticalCenter : undefined
        anchors.topMargin: Theme.spaceTiny
        anchors.verticalCenter: root.colors.length > 0 ? undefined : parent.verticalCenter
        text: {
            if (root.copied)
                return "Copied";
            if (root.hovered)
                return root.hovered.text;
            const count = root.payload?.history?.length ?? 0;
            return count === 0 ? "No colors yet" : count === 1 ? "1 color" : `${count} colors`;
        }
        elide: Text.ElideRight
        color: root.hovered || root.copied ? Theme.foreground : Theme.muted
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
    }

    Button {
        id: pick

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        tone: "accent"
        icon: "colorize"
        text: "Pick"
        onClicked: Daemon.command("colors", "start", [])
    }
}
